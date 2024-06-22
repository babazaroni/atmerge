

use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use time::Duration;


use crate::get_files_with_extension;

fn get_file_count(path: &PathBuf) -> usize {
    std::fs::read_dir(path)
        .expect("Couldn't access local directory")
        .flatten() // Remove failed
        .filter(|f|
            f.metadata().unwrap().is_file() &&
            f.path().extension().unwrap().to_str().unwrap().to_lowercase() == "csv") // Filter out directories (only consider files)
        .count()
}

fn get_file_hash(path: &PathBuf) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    //hasher.write(("abc").as_bytes());
    std::fs::read_dir(path)
        .expect("Couldn't access local directory")
        .flatten() // Remove failed
        .filter(|f|
            f.metadata().unwrap().is_file() &&
            f.path().extension().unwrap().to_str().unwrap().to_lowercase() == "csv") // Filter out directories (only consider files)
        .for_each(|f| {

            let fname = f.file_name();
            hasher.write(fname.to_str().unwrap().as_bytes());
        });
    hasher.finish()
}

pub fn start_monitor(ctx: egui::Context,tx_monitor:Sender<Option<PathBuf>>,rx_monitor:Receiver<Option<PathBuf>>) {


    tokio::spawn(async move {

        let mut force_load = false;

        let mut monitor_path = Option::<PathBuf>::None;
        let mut last_modified_time = Option::<std::time::SystemTime>::None;
        let mut last_file_count = 0;
        let mut last_file_hash = 0;

        loop {
            match rx_monitor.try_recv() {
                Ok(rx_path_msg) => {
                    if rx_path_msg.is_some() {
                        monitor_path = rx_path_msg;
                        last_modified_time = Some(std::time::SystemTime::now());
                        //last_file_count = get_file_count(&monitor_path.clone().unwrap());
                        last_file_hash = get_file_hash(&monitor_path.clone().unwrap());
                        //println!("monitor_path1: {:?}",monitor_path);
                    } else{
                        force_load = true;
                    }
                },
                Err(_) => {}
            }
 //           if let Ok(rx_path_msg) = rx_monitor.try_recv() {
 //               monitor_path = rx_path_msg;
 //               last_modified_time = Some(std::time::SystemTime::now());
 //               println!("monitor_path2: {:?}",monitor_path);

 //           }
 
            if let Some(path) = monitor_path.clone(){

                //let file_count = get_file_count(&path);

                //if file_count != last_file_count{
                //    force_load = true;
                //    last_file_count = file_count;
                //}

                let file_hash = get_file_hash(&path);
                if file_hash != last_file_hash{
                    force_load = true;
                    last_file_hash = file_hash;
                }



                let last_modified_file = std::fs::read_dir(path)
                .expect("Couldn't access local directory")
                .flatten() // Remove failed
                .filter(|f|
                    f.metadata().unwrap().is_file() &&
                    f.path().extension().unwrap().to_str().unwrap().to_lowercase() == "csv") // Filter out directories (only consider files)
                .max_by_key(|x| x.metadata().unwrap().modified().unwrap());

                if let Some(last_modified_file) = last_modified_file {
                    if let Ok(metadata) = last_modified_file.metadata(){
                        if let Ok(modified) = metadata.modified(){
                            if let Some(last_modified) = last_modified_time{
                                if modified > last_modified || force_load{
                                    force_load = false;
                                    let duration = Duration::seconds(5);  // allow main thread to make changes without causeing a loop
                                    last_modified_time = Some(std::time::SystemTime::now() + duration);
                                    let _ = tx_monitor.send(Some(last_modified_file.path()));
                                    ctx.request_repaint(); // causes continuous repaint, so we can monitor for file changes
 
                                }
                            }
                        }
                    }
                }
            
            
            }


            let ten_millis = std::time::Duration::from_millis(200);
            std::thread::sleep(ten_millis);


        }
     });
 

}