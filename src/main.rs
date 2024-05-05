#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release


use atmerge::{atmerge_self_update,load_csv, merge};
use atmerge::{prompt_for_folder, prompt_for_template,merge_excel,filter};
use atmerge::get_template;
use eframe::{egui, NativeOptions};
use egui_dock::{DockArea, DockState, Style};
use std::collections::BTreeMap;
use egui::{RichText,Color32};

use tokio::runtime::Runtime;
use std::time::Duration;

use egui::*;

use self_update::update::Release;

use egui_modal::{Icon, Modal};

use directories::{BaseDirs,UserDirs,ProjectDirs};

include!("macros.rs");



const TAB_TEMPLATE: &str = "template";
const TAB_TEST: &str = "test";
const TAB_MERGE: &str = "merge";

fn main() -> eframe::Result<()> {
    let rt = Runtime::new().expect("Unable to create Runtime");

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    // Execute the runtime in its own thread.
    // The future doesn't have to do anything. In this example, it just sleeps forever.
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        })
    });

    if let Some(base_dirs) = BaseDirs::new() {
        // want to print out "data_dir" and "data_local_dir"
        println!("BaseDirs: {:?}",base_dirs);
        println!("data_dir: {:?}",base_dirs.data_dir());
        println!("data_local_dir: {:?}",base_dirs.data_local_dir());
        println!("state_dir: {:?}",base_dirs.state_dir());
    }


    let options = NativeOptions::default();

    eframe::run_native(
        "Atmerge",
        options,
        Box::new(|cc| {         
            Box::new(MyApp::new(cc))
        }),
    )
}


#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
struct State{
    monitor_folder: Option<std::path::PathBuf>,
    merged_folder: Option<std::path::PathBuf>,
    template_file_path: Option<std::path::PathBuf>,
}

impl Default for State{
    fn default() -> Self {
        Self{
            monitor_folder: None,
            merged_folder: None,
            template_file_path: None,
        }
    }
}
 

type Title = String;


struct Atmerge {
    state: State,
    table: atmerge::Table,
    dfs: BTreeMap<Title, polars::prelude::DataFrame>,
    monitoring_folder: Option<std::path::PathBuf>,
    test_file_path: Option<std::path::PathBuf>,
    merged_file_path: Option<std::path::PathBuf>,
    rx_main: Option<std::sync::mpsc::Receiver<Option<std::path::PathBuf>>>,
    tx_main: Option<std::sync::mpsc::Sender<Option<std::path::PathBuf>>>,
    rx_update: Option<std::sync::mpsc::Receiver<bool>>,
    services_started: bool,
    update_check: bool,
    releases: Option<Vec<Release>>,
    new_release: Option<String>,
}

impl Default for Atmerge{
    fn default() -> Self {

        Self {
            state:State::default(),
            table: atmerge::Table::default(),
            dfs: BTreeMap::new(),

            monitoring_folder: None,
            test_file_path: None,
            merged_file_path: None,

            rx_main: None,
            tx_main: None,
            rx_update:None,

            services_started: false,
            update_check: false,
            releases: None,
            new_release: None,
        }
    }
}

pub fn fault(i1: i32, i2: i32) -> i32 {
    i1 / i2
}

impl Atmerge {  
    fn merge_serve(&mut self){

        if let Some(merged_folder) = self.state.merged_folder.clone(){

            if let Some(df_template) = self.dfs.get(TAB_TEMPLATE){

                //println!("df_template: {:?}",df_template.shape());

                if let Some(df_tests) = self.dfs.get(TAB_TEST){

                    if let Ok(df_filtered) = filter(Some(df_tests.clone())){



                        let df_merged = merge(df_template,&df_filtered);

                            let file_path = self.state.template_file_path.clone().unwrap();
                            let file_ext = file_path.file_name().unwrap().to_str().unwrap();

                            let file = file_ext.split(".").collect::<Vec<&str>>()[0];

                            let mut stripped_file_name = file.split("_template").collect::<Vec<&str>>()[0];
                            stripped_file_name = stripped_file_name.split("_Template").collect::<Vec<&str>>()[0];
                            stripped_file_name = stripped_file_name.split(" template").collect::<Vec<&str>>()[0];
                            stripped_file_name = stripped_file_name.split(" Template").collect::<Vec<&str>>()[0];
                            let merge_name = format!("{}_{}",stripped_file_name,"merge");


                            let merged_path_xlsx = merged_folder.join(merge_name.to_owned() + ".xlsx");

                            merge_excel(&df_template,&df_filtered,self.state.template_file_path.clone().unwrap(),&merged_path_xlsx);

                            self.merged_file_path = Some(merged_path_xlsx);                        
     
                            self.dfs.insert(TAB_MERGE.to_owned(), df_merged.clone());

                            //let merged_path_csv = merged_folder.join(stripped_file_name.to_owned() + ".csv");

                            //let dfm = & mut (df_merged.clone());

                            //self.merged_file_path = Some(merged_path_csv.clone());

                            //save_merged(dfm, Some(merged_path_csv));
                        
                    }

                }
            }
        }
    }
    fn start_services(&mut self, ui: &mut egui::Ui){

        if !self.services_started{
            self.services_started = true;

            let (tx_monitor, rx_main) = std::sync::mpsc::channel();
            let (tx_main, rx_monitor) = std::sync::mpsc::channel();
    
            let (tx_update, rx_update) = std::sync::mpsc::channel();

            self.rx_update = Some(rx_update);
            self.rx_main = Some(rx_main);
            self.tx_main = Some(tx_main);
    
            atmerge::start_update_monitor(ui.ctx().clone(),tx_update);
            atmerge::start_monitor(ui.ctx().clone(),tx_monitor,rx_monitor);
        }
    }

    fn confirm_update_modal(&mut self, ui: &mut egui::Ui) ->Modal{

        let mut update_complete_modal = Modal::new(ui.ctx(), "my_dialog");
        update_complete_modal.show_dialog();


        let confirm_update_modal = Modal::new(ui.ctx(), "my_modal");

        // What goes inside the modal
        confirm_update_modal.show(|ui| {
            // these helper functions help set the ui based on the modal's
            // set style, but they are not required and you can put whatever
            // ui you want inside [`.show()`]
            //modal.title(ui, "");
            confirm_update_modal.frame(ui, |ui| {
                confirm_update_modal.body(ui, "Are you sure you want to change versions?");
            });
            confirm_update_modal.buttons(ui, |ui| {
                // After clicking, the modal is automatically closed
                if confirm_update_modal.button(ui, "Proceed with new version").clicked() {
                    println!("Proceed with change");
                    let res = atmerge_self_update(format!("v{}",self.new_release.clone().unwrap()));
                    println!("atmerge_self_update res: {:?}",res);
                    if let Ok(_res) = res{
                        update_complete_modal.dialog()
                        //.with_title("my_function's result is...")
                        .with_body("Update Complete.  Restart to use new version")
                        .with_icon(Icon::Success)
                        .open()
                    } else {
                        update_complete_modal.dialog()
                        //.with_title("my_function's result is...")
                        .with_body(format!("Update Failed: {:?}",res))
                        .with_icon(Icon::Error)
                        .open()
                    }

                };
                if confirm_update_modal.button(ui, "Cancel change").clicked() {
                    println!("Cancel update")
                };
            }); 
        });

        confirm_update_modal

    }

    fn check_for_template(&mut self){

        if self.state.template_file_path.is_none(){
            return;
        }

        if let Some(template_file_path) = self.state.template_file_path.clone(){

            if self.dfs.get(TAB_TEMPLATE).is_none(){
                let res = get_template(Some(template_file_path));

                if let Ok(df) = res{
                    self.dfs.insert(TAB_TEMPLATE.to_owned(), df);
                    self.merge_serve();
                    return;
                } else {
                    self.state.template_file_path = None;
                }

            }
    
        }

    }
    fn check_for_monitor(&mut self){

        if self.state.monitor_folder.is_none(){
            return;
        }

        if let Some(monitor_folder) = self.state.monitor_folder.clone(){

            if self.monitoring_folder.is_none(){
                self.monitoring_folder = Some(monitor_folder.clone());
                let _result = self.tx_main.as_ref().unwrap().send(Some(monitor_folder));
            }
    
        }

    }

    fn check_keyboard(&mut self,ui: &mut egui::Ui){

        if ui.ctx().input(|i| i.key_released(Key::T)) {
            println!("\nReleased");
            let _result = self.tx_main.as_ref().unwrap().send(None);

/* 
            println!("merge_folder: {:?}",self.state.merged_folder);
            println!("templat path: {:?}",self.state.template_file_path);
            println!("monitor folder: {:?}",self.state.monitor_folder);

            println!("monitoring_folder: {:?}",self.monitoring_folder);
            println!("test_file_path: {:?}",self.test_file_path);
            println!("merged_file_path: {:?}",self.merged_file_path);
 */
        }
        if ui.ctx().input(|i| i.key_released(Key::R)) {

            let current_modifiers = ui.input(|i| i.modifiers);
            if current_modifiers.matches_exact(Modifiers::CTRL) {

                println!("\nReset");
                self.state = Default::default();
                self.dfs.remove(TAB_TEMPLATE);
                self.dfs.remove(TAB_TEST);
                self.dfs.remove(TAB_MERGE);
                self.monitoring_folder = None;
                self.test_file_path = None;
                self.merged_file_path = None;
                self.update_check = false;
                self.releases = None;
                self.new_release = None;
                ui.ctx().memory_mut(|mem| *mem = Default::default());
            }
        }

    }
    
}



impl egui_dock::TabViewer for Atmerge {
    type Tab = String;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        (&*tab).into()
    }

  

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {

        self.start_services(ui);

        self.check_for_template();

        self.check_for_monitor();

        self.check_keyboard(ui);

        if let Ok(rx_path_msg) = self.rx_main.as_ref().unwrap().try_recv() {

            println!("rx_path_msg: {:?}",rx_path_msg);

            if rx_path_msg == self.merged_file_path{
                return;
            }


            let df_result = load_csv(rx_path_msg.clone());
            if let Ok(df) = df_result {
                self.test_file_path = rx_path_msg;

                self.dfs.insert(TAB_TEST.to_owned(), df);

                self.merge_serve();

            }
        }
        if let Ok(_releases) = self.rx_update.as_ref().unwrap().try_recv() {

            if self.update_check == false{
                self.update_check = true;

                if let Ok(newest_release) = atmerge::get_releases(){
                    self.releases = Some(newest_release);
                }
            }
        }

        match tab.as_str() {
            TAB_TEMPLATE => {

                ui.horizontal_wrapped(|ui|{

                    if ui.button("Open Template File").clicked() {

                        self.state.template_file_path = prompt_for_template();

                        self.dfs.remove(TAB_TEMPLATE);
                    }
                    match self.state.template_file_path.clone(){
                    Some(template_file_path) => {
                        let tfp = format!("{:?}",template_file_path);
                        ui.label(RichText::new(tfp.trim_matches('"')).color(Color32::GREEN));
                    }
                    None => {
                        ui.label(RichText::new("No template file selected").color(Color32::RED));
                    }
                }

            });
            }
            TAB_TEST => {

                ui.horizontal_wrapped(|ui|{


                    if ui.button("Select Test Folder").clicked() {

                        let monitor_folder = prompt_for_folder();
    
                        if let Some(folder) = monitor_folder{
                            self.state.monitor_folder = Some(folder.clone());
                            self.test_file_path = None; // let check_for_monitor update monitor
                            self.dfs.remove(TAB_TEST);
                            self.dfs.remove(TAB_MERGE);
                            self.merged_file_path = None;
                        }
    
                    }
                    match self.state.monitor_folder.clone(){
                    Some(monitor_folder) => {
                        let mt = format!("{:?}",monitor_folder);
                        ui.label(RichText::new(mt.trim_matches('"')).color(Color32::GREEN));
                        match self.test_file_path.clone(){
                            Some(test_filespath) => {
                                let mt = format!("{:?}",test_filespath.file_name().unwrap());
                                ui.label(RichText::new(mt.trim_matches('"')).color(Color32::LIGHT_BLUE));
                            }
                            None => {}
                        }
    
                    }
                    None => {
                        ui.label(RichText::new("No tests folder selected").color(Color32::RED));
                    }
                }
            
            });

            }
            TAB_MERGE => {
                ui.horizontal_wrapped(|ui|{

                    if ui.button("Select Merge Folder").clicked() {

                        let merged_folder = prompt_for_folder();

                        if let Some(folder) = merged_folder{
                            self.state.merged_folder = Some(folder.clone());
                            self.merge_serve()

                        }
                    };
                    match self.state.merged_folder.clone(){
                        Some(merged_folder) => {
                            let mt = format!("{:?}",merged_folder);
                            ui.label(RichText::new(mt.trim_matches('"')).color(Color32::GREEN));
                        }
                        None => {
                            ui.label(RichText::new("No merge folder selected").color(Color32::RED));
                        }
                    }
                    match self.merged_file_path.clone(){
                        Some(merged_file_path) => {
                            let mt = format!("{:?}",merged_file_path.file_name().unwrap());
                            ui.label(RichText::new(mt.trim_matches('"')).color(Color32::LIGHT_BLUE));
                        }
                        None => {}
                    }
    

                });

 
            }
            _ => {
                ui.label("Unknown tab");
            }
        }




        if let Some(df) = self.dfs.get(tab){
            self.table.ui(ui,df);
        }
        

    }


    }


struct MyApp {
    atmerge: Atmerge,
    tree: DockState<String>,
}

// see for app persistance: rodneylab.com/trying-egui/

impl Default for MyApp {

    
    fn default() -> Self {

        let tree = DockState::new(vec![
            TAB_TEMPLATE.to_owned(), 
            TAB_TEST.to_owned(),
            TAB_MERGE.to_owned()]);

        Self { tree, atmerge: Atmerge::default()}
    }
}

impl MyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {


        let mut slf = MyApp::default();

        #[cfg(feature = "persistence")]
        if let Some(storage) = cc.storage {
            if let Some(state) = eframe::get_value(storage, eframe::APP_KEY) {
                slf.atmerge.state = state;
            }
        }

        slf
    }

    fn bar_contents(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame, _cmd: &mut Command) {

        egui::widgets::global_dark_light_mode_switch(ui);
        //ui.label(format!("Ver: {}",cargo_crate_version!()));

//        if let Some(newer_release) = self.atmerge.newer_release.clone(){
//                ui.label(format!("Newer Version Available: {}",newer_release.version));
//
//        }

        let confirm_update_modal = self.atmerge.confirm_update_modal(ui);


    //    if let Some(_new_release) = self.atmerge.new_release.clone(){

    //    }
    //    else {

            if let Some(releases) = self.atmerge.releases.clone(){

                let current_value = &mut cargo_crate_version!().to_string().clone();

                egui::ComboBox::from_id_source("Versions")
                .selected_text(format!("Current Version: {}",current_value))
                .show_ui(ui, |ui| {
                    for release in releases {
                        ui.selectable_value(current_value, release.version.clone(), release.version.clone());
                    }
                });
                if current_value != &cargo_crate_version!(){
                    self.atmerge.new_release = Some(current_value.clone());

                    confirm_update_modal.open();

                    println!("current_value: {:?}",current_value);
                }
//            }
        }


        ui.separator();
    }
}
#[derive(Clone, Copy, Debug)]
#[must_use]
enum Command {
    Nothing,
//    ResetEverything,
}

impl eframe::App for MyApp {

    #[cfg(feature = "persistence")]
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.atmerge.state);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {





        let mut cmd = Command::Nothing;
            egui::TopBottomPanel::top("wrap_app_top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().button_frame = false;
                self.bar_contents(ui, frame, &mut cmd);
            });
        });


        DockArea::new(&mut self.tree)
            .show_close_buttons(false)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut self.atmerge);
    }
}

