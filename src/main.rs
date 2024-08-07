#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release


use atmerge::{atmerge_self_update,load_csv,save_csv,fix_quotes,ReportFormat,get_files_with_extension,compare_with_trailing_number};
use atmerge::{prompt_for_folder, prompt_for_template,merge_excel_append,merge_excel_format,filter_fails,get_paths_from_part_folder,search_for_format_file};
use atmerge::get_df_from_xlsx;
use calamine::Data;
use eframe::{egui, NativeOptions};
use egui_dock::{DockArea, DockState, Style};
use polars::frame::DataFrame;
use polars_io::csv::CsvWriter;
use std::collections::BTreeMap;
use std::path::PathBuf;
use egui::{RichText,Color32};

use tokio::runtime::Runtime;
use std::time::Duration;

use egui::*;

use self_update::update::Release;

use egui_modal::{Icon, Modal};

use directories::{BaseDirs,UserDirs,ProjectDirs};

use win_beep;

use std::backtrace::Backtrace;
use std::{env, fs};

#[macro_use]
extern crate crashreport;

include!("macros.rs");



const TAB_TEMPLATE: &str = "template";
const TAB_TEST: &str = "results";
const TAB_MERGE: &str = "report";

fn main() -> eframe::Result<()> {
    //crashreport!();

 #[cfg(debug_assertions)]
    std::panic::set_hook(Box::new(|_panic_info| {

        let backtrace = std::backtrace::Backtrace::force_capture();

        let mut path = env::current_exe().unwrap();

        path.set_file_name("atmerge_crash_report.txt");


        eprintln!("My backtrace: {:#?}",backtrace);
        let _res = fs::write(path,format!("{:#?}",backtrace));

    }));
 

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
/*

    if let Some(base_dirs) = BaseDirs::new() {
        // want to print out "data_dir" and "data_local_dir"
        println!("BaseDirs: {:?}",base_dirs);
        println!("data_dir: {:?}",base_dirs.data_dir());
        println!("data_local_dir: {:?}",base_dirs.data_local_dir());
        println!("state_dir: {:?}",base_dirs.state_dir());
    }
*/


    let mut options = NativeOptions::default();
    
    options.centered = true;  // works on Windows to keep the window from wandering off screen
    
    
    eframe::run_native(
        "Atmerge",
        options,
        Box::new(|cc| {    
            cc.egui_ctx.set_visuals(egui::Visuals::dark());  
            Box::new(MyApp::new(cc))
        }),
    )

}


pub fn divide(a: f64, b: f64) -> f64 {
    let d: Option<u32> = None;
    println!("Dividing {} by {}", a, b);
    let c = a / b;
    println!("Result: {}", c);
    d.unwrap();
    c
}


#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Clone, Debug)]
struct State{
    monitor_folder: Option<std::path::PathBuf>,
    merged_folder: Option<std::path::PathBuf>,
    template_file_path: Option<std::path::PathBuf>,
    part_folder: Option<std::path::PathBuf>,
}


impl Default for State{
    fn default() -> Self {
        Self{
            monitor_folder: None,
            merged_folder: None,
            template_file_path: None,
            part_folder: None,
        }
    }
}
 

type Title = String;


struct Atmerge {
    state: State,
    table: atmerge::Table,
    dfs: BTreeMap<Title, polars::prelude::DataFrame>,
    monitoring_folder: Option<std::path::PathBuf>,
    test_file_path: Option<Vec<std::path::PathBuf>>,
    test_file_counts: Vec<usize>,
    merged_file_path: Option<std::path::PathBuf>,
    rx_main: Option<std::sync::mpsc::Receiver<Option<std::path::PathBuf>>>,
    tx_main: Option<std::sync::mpsc::Sender<Option<std::path::PathBuf>>>,
    rx_update: Option<std::sync::mpsc::Receiver<bool>>,
    services_started: bool,
    update_check: bool,
    releases: Option<Vec<Release>>,
    new_release: Option<String>,
    show_versions: bool,
    update_state: UpdateState,
    update_results: Result<(), Box<dyn ::std::error::Error>>

}

impl Default for Atmerge{
    fn default() -> Self {

        Self {
            state:State::default(),
            table: atmerge::Table::default(),
            dfs: BTreeMap::new(),

            monitoring_folder: None,
            test_file_path: None,
            test_file_counts: Vec::new(),
            merged_file_path: None,

            rx_main: None,
            tx_main: None,
            rx_update:None,

            services_started: false,
            update_check: false,
            releases: None,
            new_release: None,
            show_versions: false,
            update_state: UpdateState::CONFIRMUPDATE,
            update_results: Ok(()),
        }
    }
}
#[derive(Clone, Copy, Debug)]
enum UpdateState {
    CONFIRMUPDATE,
    UPDATING1,
    UPDATING2,
    RESULTS
}

impl Atmerge { 
    #[cfg(target_os = "windows")]
    fn beep(&mut self){
        win_beep::beep_with_hz_and_millis(440, 100);
    }

    #[cfg(target_os = "linux")] 
    fn beep(&mut self){
        println!("beep");
    }
    
    fn merge_serve(&mut self){

        if let Some(merged_folder) = self.state.merged_folder.clone(){

            if let Some(df_template) = self.dfs.get(TAB_TEMPLATE){

                //println!("df_template: {:?}",df_template.shape());

                if let Some(df_tests) = self.dfs.get(TAB_TEST){


                    //if let Ok(df_filtered) = filter_fails(Some(df_tests.clone())){

                        let file_path = self.state.template_file_path.clone().unwrap();
                        let file_ext = file_path.file_name().unwrap().to_str().unwrap();

                        let file = file_ext.split(".").collect::<Vec<&str>>()[0];

                        let mut stripped_file_name = file.split("_template").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split("_Template").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split(" template").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split(" Template").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split("_data").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split("_Data").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split(" data").collect::<Vec<&str>>()[0];
                        stripped_file_name = stripped_file_name.split(" Data").collect::<Vec<&str>>()[0];
                        let merge_name = format!("{} {}",stripped_file_name,"Report");
                        //let merge_name = stripped_file_name;


                        let merged_path_xlsx = merged_folder.join(merge_name.to_owned() + ".xlsx");

                        let format_file:Option<PathBuf> = search_for_format_file(&self.state.template_file_path.as_ref().unwrap().parent().unwrap());
 

                        if let Some(format_file) = format_file{
                            let report_format = ReportFormat::new(&format_file);

                            merge_excel_format(&df_tests,self.state.template_file_path.clone().unwrap(),&merged_path_xlsx,&report_format);
                        } else {
                            merge_excel_append(&df_template,&df_tests,self.state.template_file_path.clone().unwrap(),&merged_path_xlsx);
                        }
                        self.merged_file_path = Some(merged_path_xlsx);

                        let df_merged = get_df_from_xlsx(self.merged_file_path.clone());  

                        if let Ok(df) = df_merged{
                            self.dfs.insert(TAB_MERGE.to_owned(), df);
                            self.beep();
                        }                      

                            //let merged_path_csv = merged_folder.join(stripped_file_name.to_owned() + ".csv");

                            //let dfm = & mut (df_merged.clone());

                            //self.merged_file_path = Some(merged_path_csv.clone());

                            //save_merged(dfm, Some(merged_path_csv));
                        
                    //}

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

        confirm_update_modal.show(|ui| {

            match self.update_state {

                UpdateState::CONFIRMUPDATE => {
                    confirm_update_modal.frame(ui, |ui| {
                        confirm_update_modal.body(ui, "Are you sure you want to change versions?");
                    });
                    confirm_update_modal.buttons(ui, |ui| {
                        if confirm_update_modal.button(ui, "Proceed with new version").clicked() {
                            self.update_state = UpdateState::UPDATING1;
                            confirm_update_modal.open();
                            println!("Proceed with change");
                            ui.ctx().request_repaint();
    
                        } else{
                            if confirm_update_modal.button(ui, "Cancel update").clicked() {
                                //confirm_update_modal.close();
                                println!("Cancel update")
                            };
                        }
                    }); 
                }
                UpdateState::UPDATING1 => {
                    println!("Updating......");
                    confirm_update_modal.frame(ui, |ui| {
                        confirm_update_modal.body(ui, "Updating......");
                    });
                    self.update_state = UpdateState::UPDATING2;
                }
                UpdateState::UPDATING2 => {
                    self.update_results = atmerge_self_update(format!("v{}",self.new_release.clone().unwrap()));
                    self.update_state = UpdateState::RESULTS;
                    ui.ctx().request_repaint();
                }
                UpdateState::RESULTS => {
                    if let Ok(_res) = self.update_results{
                        update_complete_modal.dialog()
                        //.with_title("my_function's result is...")
                        .with_body("Update Complete.  Restart to use new version")
                        .with_icon(Icon::Success)
                        .open();
                    } else {
                        update_complete_modal.dialog()
                        //.with_title("my_function's result is...")
                        .with_body(format!("Update Failed: {:?}",self.update_results))
                        .with_icon(Icon::Error)
                        .open();
                    }
                    confirm_update_modal.close();
                }
            }

        });

        confirm_update_modal

    }

    fn check_for_template(&mut self){

        if self.state.template_file_path.is_none(){
            return;
        }

        if let Some(template_file_path) = self.state.template_file_path.clone(){

            if self.dfs.get(TAB_TEMPLATE).is_none(){


                let res = get_df_from_xlsx(Some(template_file_path));

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

    fn reset(&mut self,ui: &mut egui::Ui){
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

    fn check_ctrl_keys(&mut self,ui: &mut egui::Ui){

        let mut current_modifiers = ui.input(|i| i.modifiers);
        current_modifiers.shift = false;
        if current_modifiers.matches_exact(Modifiers::CTRL) {
            if ui.ctx().input(|i| i.key_released(Key::R)) {self.reset(ui);}
            if ui.ctx().input(|i| i.key_released(Key::V)) {self.show_versions = !self.show_versions;}
            if ui.ctx().input(|i| i.key_released(Key::P)) {panic!("panic in keyboard");}
        }
    }

    fn check_keyboard(&mut self,ui: &mut egui::Ui){

        if ui.ctx().input(|i| i.key_released(Key::T)) {
            let _result = self.tx_main.as_ref().unwrap().send(None);
        }
        self.check_ctrl_keys(ui);
    }

    fn process_result_folder(&mut self){
        let mut csv_list = get_files_with_extension(self.state.monitor_folder.as_ref().unwrap(),"csv");

        //let csv_list = Ok(vec!(rx_path_msg.clone().unwrap()));


        if let Ok(csv_list) = csv_list.as_mut(){

            csv_list.sort_by(|a, b|
                compare_with_trailing_number(a.file_name().unwrap().to_str().unwrap(),b.file_name().unwrap().to_str().unwrap())
            );

            let mut dfm = DataFrame::default();
            let mut test_counts: Vec<usize> = Vec::new();

            for csv_path in csv_list.clone().iter(){

                let df_result = load_csv(Some(csv_path.clone()));
                if let Ok(df) = df_result {

                     let format_file:Option<PathBuf> = search_for_format_file(&self.state.template_file_path.as_ref().unwrap().parent().unwrap());

                    if let Some(format_file) = format_file{
                        let report_format = ReportFormat::new(&format_file);

                    //filter_fails
                        if let (Ok(df_filtered),test_count) = filter_fails(Some(df.clone()),&report_format){

                            dfm = dfm.vstack(&df_filtered).unwrap();

                            test_counts.push(test_count);

                            let dfm = &mut df_filtered.clone();

                            save_csv(dfm,Some(csv_path.clone()));

                            fix_quotes(&csv_path);
            
                        }
                    }
                }

            }

            self.dfs.insert(TAB_TEST.to_owned(), dfm);

            self.test_file_path = Some(csv_list.clone());
            self.test_file_counts = test_counts;

            self.merge_serve();
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

            //println!("rx_path_msg: {:?}",rx_path_msg);

            if rx_path_msg == self.merged_file_path{
                return;
            }

            self.process_result_folder();


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

                    if ui.button(format!("Open {TAB_TEMPLATE} file")).clicked() {

                        self.state.template_file_path = prompt_for_template();

                        self.dfs.remove(TAB_TEMPLATE);
                    }
                    match self.state.template_file_path.clone(){
                        Some(template_file_path) => {
                            let tfp = format!("{:?}",template_file_path.file_name().unwrap());
                            let full = format!("{:?}",template_file_path);
                            ui.label(RichText::new(tfp.trim_matches('"')).color(Color32::GREEN)).on_hover_text(full.trim_matches('"'));
                        }
                        None => {
                            ui.label(RichText::new(format!("No {TAB_TEMPLATE} file selected")).color(Color32::RED));
                        }
                    }

                });
            }
            TAB_TEST => {

                ui.horizontal_wrapped(|ui|{


                    if ui.button(format!("Select {TAB_TEST} folder")).clicked() {

                        let monitor_folder = prompt_for_folder();
    
                        if let Some(folder) = monitor_folder{
                            self.state.monitor_folder = Some(folder.clone());
                            self.test_file_path = None; // let check_for_monitor update monitor
                            self.dfs.remove(TAB_TEST);
                            self.dfs.remove(TAB_MERGE);
                            self.merged_file_path = None;
                            self.process_result_folder();
                        }
    
                    }
                    match self.state.monitor_folder.clone(){
                        Some(monitor_folder) => {
                            let mt = format!("{:?}",monitor_folder.file_name().unwrap());
                            let full = format!("{:?}",monitor_folder);
                            ui.label(RichText::new(mt.trim_matches('"')).color(Color32::GREEN)).on_hover_text(full.trim_matches('"'));
                            match self.test_file_path.clone(){
                                Some(test_filespath) => {

                                    let mut file_names = String::from("");
                                    let mut base_count = 1;
                                    for (path,count) in test_filespath.iter().zip(self.test_file_counts.iter()){
                                        if file_names.len()>0{
                                            file_names  = format!("{},",file_names.clone());
                                        }

                                        
                                        let next_file = format!("{:?}",path.file_name().unwrap()).trim_matches('"').to_string();

                                        let first = next_file.split('.').collect::<Vec<&str>>()[0];
                                        //println!("first: {}",first.trim_matches('"'));
                                        //file_names = format!("{}{}",file_names,next_file.trim_matches('"'));
                                        file_names = format!("{}{}({},{})",file_names,first,count,base_count);
                                        base_count += count;

                                    }

                                    //let mt = format!("{:?}",test_filespath.file_name().unwrap());
                                    ui.label(RichText::new(file_names).color(Color32::LIGHT_BLUE));
                                }
                                None => {}
                            }
                        }
    
    
                        None => {
                            ui.label(RichText::new(format!("No {TAB_TEST} folder selected")).color(Color32::RED));
                        }
                    }
            
                });
            }

            TAB_MERGE => {
                ui.horizontal_wrapped(|ui|{

                    if ui.button(format!("Select {TAB_MERGE} folder")).clicked() {

                        let merged_folder = prompt_for_folder();

                        if let Some(folder) = merged_folder{
                            self.state.merged_folder = Some(folder.clone());
                            self.merge_serve();
                        }
                    };
                    match self.state.merged_folder.clone(){
                        Some(merged_folder) => {
                            let mt = format!("{:?}",merged_folder.file_name().unwrap());
                            let full = format!("{:?}",merged_folder);
                            ui.label(RichText::new(mt.trim_matches('"')).color(Color32::GREEN)).on_hover_text(full.trim_matches('"'));
                        }
                        None => {
                            ui.label(RichText::new(format!("No {TAB_MERGE} folder selected")).color(Color32::RED));
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

// for tab title rich text
// TabViewer::title returns any WidgetText so you can just create a RichText and convert it with .into() on return

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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {


        let slf = MyApp::default();

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

        if ui.button("Select Part Folder").clicked() {
            let part_folder = prompt_for_folder();

            if part_folder.is_some(){

                self.atmerge.reset(ui);


                let folders = get_paths_from_part_folder(&part_folder);

                if folders.0.is_some() && folders.1.is_some() && folders.2.is_some(){

                    self.atmerge.state.monitor_folder = folders.0;
                    self.atmerge.state.merged_folder = folders.1;
                    self.atmerge.state.template_file_path = folders.2;

                    self.atmerge.state.part_folder = part_folder.clone();

                    self.atmerge.process_result_folder();
                }

            }
        }
        match self.atmerge.state.part_folder.clone(){
            Some(part_folder_path) => {
                let pfp = format!("{:?}",part_folder_path.file_name().unwrap());
                let full = format!("{:?}",part_folder_path);
                ui.label(RichText::new(pfp.trim_matches('"')).color(Color32::GREEN)).on_hover_text(full.trim_matches('"'));
            }
            None => {
                ui.label(RichText::new("No part folder selected").color(Color32::RED));
            }
        }

        if self.atmerge.show_versions{

            let current_value = &mut cargo_crate_version!().to_string().clone();

            egui::ComboBox::from_id_source("Versions")
            .selected_text(format!("Current Version: {}",current_value))
            .show_ui(ui, |ui| {
                if let Some(releases) = self.atmerge.releases.clone(){
                    for release in releases {
                        ui.selectable_value(current_value, release.version.clone(), release.version.clone()).on_hover_text_at_pointer(release.body.clone().unwrap());
                    }
                }
            });
            if current_value != &cargo_crate_version!(){
                self.atmerge.new_release = Some(current_value.clone());

                self.atmerge.update_state = UpdateState::CONFIRMUPDATE;
                confirm_update_modal.open();


                println!("current_value: {:?}",current_value);
            }
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

