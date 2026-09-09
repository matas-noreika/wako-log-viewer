// import the gui libraries
// egui - UI toolkit
// eframe - GUI framework
use eframe::egui;

//define a struct that hold app data and functionalities
struct App;

//implement the update method for app interface on app
impl eframe::App for App {
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
    ) {
        ui.heading("Hello World!");
        ui.label("Hello from egui!");
    }
}

//entry point
fn main() -> eframe::Result<()> {
    // creates option parameters using default properties
    let options = eframe::NativeOptions::default();
 
    //main entry point of GUI
    eframe::run_native(
        "Hello Window", // name on window
        options, // pass our options
        Box::new(|_cc| Ok(Box::new(App))), // Ok sets the Result object passed back to the value as
        // our new created object. It uses a rust closure |..| {..}
    )
}
