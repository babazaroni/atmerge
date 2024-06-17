#![warn(clippy::all, rust_2018_idioms)]

mod table;
pub use table::Table;
mod utils;
pub use utils::{prompt_for_csv,prompt_for_folder,clean_df_val};
pub use utils::{load_csv,merge,save_merged,merge_excel_append,merge_excel_format,filter,prompt_for_template,get_df_from_xlsx,get_paths_from_part_folder,get_format_file};
mod monitor;
pub use monitor::start_monitor;
mod update;
pub use update::{atmerge_self_update,start_update_monitor,get_releases,get_newer_release};

