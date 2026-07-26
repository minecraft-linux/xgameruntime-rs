use eframe::egui;
use winit::platform::windows::EventLoopBuilderExtWindows;

#[derive(Default)]
struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Move the window, then move the mouse");


            ui.label(format!(
                "pointer: {:?}",
                ui.input(|input| {
                    input.pointer.hover_pos()
                })
            ));
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),

        ..Default::default()
    };

    eframe::run_native(
        "eframe mouse test",
        options,
        Box::new(|_| Ok(Box::<App>::default())),
    )
}