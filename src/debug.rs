use crate::graphics::{self, renderdoc::RenderDoc, Graphics};
use crate::logger;
use crate::ui;
use crossbeam::channel::{self, Receiver, Sender};
use gfx_hal::{Backend, PresentMode};
use imgui::{im_str, ImString, Ui};

const NETWORK_HISTORY_LENGTH: usize = 256;
const FRAME_TIME_HISTORY_LENGTH: usize = 256;

pub const NETWORK_STATS_RATE: f32 = 0.1;

#[derive(Default, Debug, Copy, Clone)]
pub struct NetworkStats {
    /// Number of bytes sent since the last recorded stats.
    pub bytes_out: u32,
    /// Number of bytes received since the last recorded stats.
    pub bytes_in: u32,
}

/// State and options related to the debug window.
#[derive(Clone)]
pub struct DebugState {
    /// Whether to draw an overlay showing the most recently received
    /// snapshot.
    ///
    /// This is useful to debug the difference between the
    /// interpolated visual positions and the raw snapshots.
    pub draw_latest_snapshot: bool,
    /// The delay in multiples of the snapshot rate to buffer
    /// snapshots for interpolation.
    ///
    /// Increasing this will make things smoother in the presence of
    /// packet loss or jitter, but will increase visual latency.
    pub interpolation_delay: f32,
    pub network_tx: Sender<NetworkStats>,
    network_rx: Receiver<NetworkStats>,
    bandwidth_in_history: [f32; NETWORK_HISTORY_LENGTH],
    bandwidth_out_history: [f32; NETWORK_HISTORY_LENGTH],
    frame_time_history: [f32; FRAME_TIME_HISTORY_LENGTH],
}

impl Default for DebugState {
    fn default() -> DebugState {
        let (network_tx, network_rx) = channel::bounded(32);
        DebugState {
            draw_latest_snapshot: false,
            interpolation_delay: 1.5,
            network_tx,
            network_rx,
            bandwidth_in_history: [0.0; NETWORK_HISTORY_LENGTH],
            bandwidth_out_history: [0.0; NETWORK_HISTORY_LENGTH],
            frame_time_history: [0.0; FRAME_TIME_HISTORY_LENGTH],
        }
    }
}

impl DebugState {
    /// Draws the debug window into imgui.
    pub fn ui<'a, B: Backend>(
        &mut self,
        ui: &Ui<'a>,
        graphics: &mut Graphics<B>,
        renderdoc: &mut RenderDoc,
        frame_time: f32,
    ) {
        // Convert frame_time to ms.
        let frame_time = frame_time * 1000.0;

        // Log network statistics.
        let size = self.network_rx.len();
        if size > 0 {
            // Copy elements to make room for new ones.
            self.bandwidth_in_history.copy_within(size.., 0);
            self.bandwidth_out_history.copy_within(size.., 0);
            let start = NETWORK_HISTORY_LENGTH - size;
            for (i, stats) in self.network_rx.try_iter().enumerate() {
                let bandwidth_in = stats.bytes_in as f32 / NETWORK_STATS_RATE;
                let bandwidth_out = stats.bytes_out as f32 / NETWORK_STATS_RATE;
                // Convert to KB
                self.bandwidth_in_history[start + i] = bandwidth_in / 1000.0;
                self.bandwidth_out_history[start + i] = bandwidth_out / 1000.0;
            }
        }

        // Log the frame time.
        self.frame_time_history.copy_within(1.., 0);
        *self.frame_time_history.last_mut().unwrap() = frame_time;

        ui.window(im_str!("Debug")).build(|| {
            ui.tree_node(im_str!("Networking")).build(|| {
                let bandwidth_in = *self.bandwidth_in_history.last().unwrap();
                let bandwidth_out = *self.bandwidth_out_history.last().unwrap();

                ui.plot_lines(
                    im_str!("Bandwidth in"),
                    &self.bandwidth_in_history,
                )
                .scale_max(8.0)
                .scale_min(0.0)
                .overlay_text(&ImString::new(format!(
                    "{:.2} KB/s",
                    bandwidth_in
                )))
                .build();
                ui.plot_lines(
                    im_str!("Bandwidth out"),
                    &self.bandwidth_out_history,
                )
                .scale_max(8.0)
                .scale_min(0.0)
                .overlay_text(&ImString::new(format!(
                    "{:.2} KB/s",
                    bandwidth_out
                )))
                .build();

                ui.checkbox(
                    im_str!("Draw latest snapshot"),
                    &mut self.draw_latest_snapshot,
                );

                ui.input_float(
                    im_str!("Interpolation delay"),
                    &mut self.interpolation_delay,
                )
                .build();
            });

            ui.tree_node(im_str!("Graphics")).build(|| {
                ui.plot_lines(im_str!("Frame time"), &self.frame_time_history)
                    .scale_max(1000.0 / 20.0)
                    .scale_min(0.0)
                    .overlay_text(&ImString::new(format!(
                        "{:.2} ms",
                        frame_time
                    )))
                    .build();

                let mut present_mode = graphics.present_mode();
                if ui::enum_combo(
                    &ui,
                    im_str!("Present mode"),
                    &mut present_mode,
                    &[
                        im_str!("immediate"),
                        im_str!("relaxed"),
                        im_str!("fifo"),
                        im_str!("mailbox"),
                    ],
                    &[
                        PresentMode::Immediate,
                        PresentMode::Relaxed,
                        PresentMode::Fifo,
                        PresentMode::Mailbox,
                    ],
                    4,
                ) {
                    graphics.set_present_mode(present_mode);
                }

                if ui.small_button(im_str!("Capture frame")) {
                    graphics::renderdoc::trigger_capture(renderdoc, 1);
                }
            });

            ui.tree_node(im_str!("Logger")).build(|| {
                logger::LOGGER.ui(&ui);
            });
        });
    }
}
