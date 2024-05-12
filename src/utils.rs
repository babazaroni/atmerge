use polars_core::prelude::*;

use polars_io::prelude::*;

use std::io::prelude::*;
use std::{fs, i64};

use std::fs::File;
use std::io::{self, BufRead, LineWriter};
use std::path::{Path,PathBuf};





fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
pub fn get_max_columns(path:&String) -> Option<usize>{
    if let Ok(lines) = read_lines(path) {

        let mut max_columns = 0;
        for line in lines.flatten() {
            let count = line.split(",").count();
            if count>max_columns{
                max_columns = count;
            }
        }
        return Some(max_columns);
    }
    None
}

pub fn fix_broken_csv(path:&String, fixed_path:&str){
    let max_columns = get_max_columns(path).unwrap();
    if max_columns>1{
        let mut new_lines = Vec::new();
        if let Ok(lines) = read_lines(path) {
            for line in lines.flatten() {
                let count = line.split(",").count();
                if count<max_columns{
                    let mut new_line = line.clone();
                    for _ in 0..(max_columns-count){
                        new_line.push_str(",");
                    }
                    new_lines.push(new_line);
                }else{
                    new_lines.push(line);
                }
            }
        }
        let file = File::create(fixed_path).unwrap();
        let mut file = LineWriter::new(file);
        for line in new_lines{
            file.write_all(line.as_bytes()).unwrap();
            file.write_all(b"\n").unwrap();
        }
    }
}

pub fn read_polars_csv(path:&String) -> PolarsResult<DataFrame> {

    fix_broken_csv(path,"delete.csv");

    let rval = CsvReader::from_path("delete.csv")?
            .with_quote_char(Some(b'\''))
            .has_header(false)
            .infer_schema(None)//None: Use all values to infer type
            .with_missing_is_null(true) // false: String columns convert nulls to ""  true: leave as null
            .finish();

    fs::remove_file("delete.csv").unwrap();

    rval
}

pub fn prompt_for_folder() ->Option<std::path::PathBuf>{
    let path = rfd::FileDialog::new()
    .set_directory(".")
    .pick_folder();

    if let Some(picked_path) = path {
        return Some(picked_path);
    }
    None
}

pub fn get_paths_from_part_folder(path: Option<PathBuf>) -> (Option<std::path::PathBuf>,Option<std::path::PathBuf>,Option<std::path::PathBuf>){

    let mut result_path: Option<PathBuf> = None;
    let mut report_path: Option<PathBuf> = None;
    let mut template_path: Option<PathBuf> = None;


    if let Some(picked_path) = path {

        let entries = std::fs::read_dir(picked_path)
        .unwrap();

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                    let path_str = path.to_str().unwrap().to_ascii_lowercase();
                if path.is_dir(){
                    if result_path.is_none() &&  path_str.contains("result"){
                        result_path = Some(path.clone());
                    }
                    if report_path.is_none() &&  path_str.contains("report"){
                        report_path = Some(path.clone());
                    }
                }
                if path.is_file(){
                    if template_path.is_none() &&  path_str.contains("template"){
                        template_path = Some(path.clone());
                    }
                }
            }


        }
    }
    (result_path,report_path,template_path)
}

pub fn load_csv(path:Option<PathBuf>) -> PolarsResult<DataFrame> {
    if let Some(picked_path) = path {

        let polars_result: PolarsResult<DataFrame> = read_polars_csv(&picked_path.display().to_string());

        //polars_result.as_ref().unwrap().get_column_names().iter().for_each(|x| println!("{}",x));

        return polars_result;

    }
    Err(PolarsError::NoData("No file selected".into()))

}


pub fn prompt_for_csv() -> PolarsResult<DataFrame> {

    let path = rfd::FileDialog::new()
    .set_directory(".")
    //.add_filter("CSV",&["csv"]).pick_file();
    .pick_folder();

    load_csv(path)

}

pub fn clean_df_val(df_val:AnyValue<'_>)->String{
    let null_filtered = match df_val{
        polars::datatypes::AnyValue::Null => String::from(""),
        polars::datatypes::AnyValue::Float32(df_val) => format!("{:.2}",df_val),
        polars::datatypes::AnyValue::Float64(df_val) => {if df_val.is_nan(){String::from("")}
                                                              else{format!("{:.2}",df_val)}},
        _ => format!("{}",df_val),
    };
    let trimmed: &str = null_filtered.trim_matches('"'); // remove quotes
    String::from(trimmed)
}
use calamine::{Reader, open_workbook, Xlsx, Data};

pub fn prompt_for_template()->Option<PathBuf>{
    let path = rfd::FileDialog::new()
    .set_directory(".")
    .add_filter("XLSX",&["xls*","csv"]).pick_file();
    return path
}

pub fn get_df_from_xlsx(path:Option<PathBuf>) -> PolarsResult<DataFrame> {

    if let Some(picked_path) = path {
        if picked_path.exists() == false{
            return Err(PolarsError::NoData("File does not exist".into()));}

        if picked_path.extension().unwrap() == "csv"{
            let res = load_csv(Some(picked_path.clone()));
            if let Ok(df) = res{
                return Ok(df);
            }
            return  Err(PolarsError::NoData("No file selected".into()));
        }
        

        let mut workbook: Xlsx<_> = open_workbook(&picked_path).expect("Cannot open file"); 
        workbook.load_tables().expect("Cannot load tables");


        let sheets = workbook.sheet_names().to_owned();
        let first_sheet = sheets[0].clone();

        if let Ok(range) = workbook.worksheet_range(&first_sheet) {
            //let total_cells = range.get_size().0 * range.get_size().1;
            let non_empty_cells: usize = range.used_cells().count();
            //println!("Found {} cells in '{}', including {} non empty cells",total_cells, first_sheet,non_empty_cells);
            // alternatively, we can manually filter rows
            assert_eq!(non_empty_cells, range.rows()
                .flat_map(|r| r.iter().filter(|&c| c != &Data::Empty)).count());

            // Range is oriented by rows, but dataframe needs columns so convert

            let mut columns: Vec<Vec<String>> = (0..range.width()).map(|_x| Vec::new()).collect();

            for r in range.rows() {
                for (x,c) in r.iter().enumerate() {
                    let cv = match c {
                        Data::Empty => String::from( ""),
                        Data::String(s) => s.trim_matches('"').to_string(),
                        Data::Float(f) => format!("{}",f),
                        Data::Int(i) => format!("{}",i),
                        Data::Bool(b) => format!("{}",b),
                        Data::DateTime(dt) => format!("{}",dt),
                        Data::DateTimeIso(dt) => format!("{}",dt),
                        Data::Error(e) => format!("{}",e),
                        _ => String::from(""),
                    };
                    columns[x].push(cv.clone());
                }
            }

            let series : Vec<Series> = columns.iter().enumerate().map(|(i,x)| Series::new(&format!("column_{}",i+1),x)).collect();

            let df = DataFrame::new(series).unwrap();

            return Result::Ok(df);
        }
    }

    Err(PolarsError::NoData("No file selected".into()))

}
pub fn prompt_for_excel() -> PolarsResult<DataFrame> {

    let path = prompt_for_template();

    get_df_from_xlsx(path)

}

fn get_vec_columns(df: &DataFrame) -> Vec<Series>{
    let mut columns = Vec::new();
    let names = df.get_column_names();
    for i in 0..df.width(){
        if let Ok(column) = df.column(names[i]){
            columns.push(column.clone());
        }
    }
    columns
}

fn pad_column(df: &DataFrame,target_width:usize) -> DataFrame{

    let mut new_columns = get_vec_columns(df);

    for i in df.width()+1..=target_width{
        new_columns.push(Series::new(&format!("column_{}",i),vec!["";df.height()]));
    }

    DataFrame::new(new_columns).unwrap()
}


pub fn merge(df1:&DataFrame, df2:&DataFrame) -> DataFrame {

    let w = df1.width().max(df2.width());

    let df1 = pad_column(df1,w);
    let df2 = pad_column(df2,w);

    let df = df1.vstack(&df2).unwrap();

    return df;            

}

static ASCII_UPPERCASE: [char;26] = ['A','B','C','D','E','F','G','H','I','J','K','L','M','N','O','P','Q','R','S','T','U','V','W','X','Y','Z'];

pub fn merge_excel(df_template:&DataFrame,df_tests:&DataFrame,source_path: PathBuf,dest_path: &PathBuf){

    let mut book = umya_spreadsheet::reader::xlsx::read(source_path).unwrap();

    //let sheet_names = workbook.sheet_names().to_owned();
    //let first_sheet = sheet_names[0].clone();

    let mut test_row: Option<usize> = None;



    for row in 1..50{
        let header = format!("A{}",row);
        let v = book
            .get_sheet_mut(&0)
            .unwrap()
            .get_cell(header);

        if let Some(cell) = v{
            let val = cell.get_value();
            if val.to_uppercase() == "TESTS" {
                //test_row = Some(row);
                break;
            }
        }
    }

    if test_row.is_none(){
        test_row = Some(df_template.shape().0 + 4);
        println!("No 'Tests' row found in template, using {}",test_row.unwrap());
    }

    let columns = get_vec_columns(&df_tests);

    for (cnum,column) in columns.iter().enumerate(){
        let column = column.rechunk();  // do this or iterating will crash
        for (rnum,entry) in column.iter().enumerate(){
            let val = match entry{
                polars::datatypes::AnyValue::Utf8(val) => val.trim_matches( '"'),
                _ => ""
            };
            let header = format!("{}{}",ASCII_UPPERCASE[cnum].to_string(),rnum + test_row.unwrap());
            let _ = book
            .get_sheet_mut(&0)
            .unwrap()
            .get_cell_mut(header)
            .set_value(val);
        }
    }

    edit_excel(&mut book,test_row.unwrap());

    let _ = umya_spreadsheet::writer::xlsx::write(&mut book, dest_path);


}

fn edit_excel(book: &mut umya_spreadsheet::Spreadsheet,test_row:usize){
    let mut row = test_row + 5;

    let mut state = PARSESTATES::START;

    let mut test_num = 1;

    loop {
        let header = format!("A{}",row);
        let v = book
            .get_sheet_mut(&0)
            .unwrap()
            .get_cell_mut(header);



            let mut cell_value = v.get_value().to_uppercase();
            cell_value = cell_value.trim_matches('"').to_string();
            match state{
                PARSESTATES::START => {
                    if cell_value == ""{
                        break;
                    }
                    v.set_value(format!("{}",test_num));
                    test_num += 1;
                    state = PARSESTATES::END;
                },
                PARSESTATES::END => {
                    v.set_value("");
                    if cell_value == ""{
                        state = PARSESTATES::START;
                    }
                }
            }
        

        row += 1;
    
    }
}

fn get_pass_fail_column(columns:Vec<Series>) -> Option<Series>{

    for column in columns{
        let column = column.rechunk();  // do this or iterating will crash

        for entry in column.iter(){

            let val = match entry{
                polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                _ => "_"

            };

            if val == "PASS/FAIL"{
                //println!("Found PASS/FAIL column");
                return Some(column.clone());
            }

        }
    }
    None
}
#[derive(Debug)]
enum PARSESTATES {
    START,
    END

}
pub fn filter(df:Option<DataFrame>) -> PolarsResult<DataFrame> {

    let mut parse_state = PARSESTATES::START;
    let mut slice_start = 0;

    let mut slice_rows: Vec<(usize,usize)> = Vec::new();

    let mut fail_count = 0;

    //println!("filter: {:?}",df);


    if let Some(df) = df {

        let columns = get_vec_columns(&df);
        let pf_column = get_pass_fail_column(columns);

        //println!("pf_column: {:?}",pf_column);

        if let Some(pf_column) = pf_column{
            for (x,entry) in pf_column.iter().enumerate(){


                let val = match entry{
                    polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                    _ => "_"

                };

                    //println!("filter: {:?} {:?} {:?}",parse_state,x,val);

                    match parse_state{
                        PARSESTATES::START => {
                            match val{
                                "PASS" => {
                                    fail_count = 0;
                                    //println!("pushing slice pass: {} {}",slice_start,x - slice_start);
                                    slice_rows.push((slice_start,x - slice_start));
                                    slice_start = x;
                                    parse_state = PARSESTATES::END;
                                },
                                "FAIL" => {
                                    fail_count = 1;
                                    //println!("pushing slice fail: {} {}",slice_start,x - slice_start);
                                    slice_rows.push((slice_start,x - slice_start));
                                    slice_start = x;
                                    parse_state = PARSESTATES::END;
                                },
                                _ => {}
                            }
                        },
                        PARSESTATES::END => {
                            match val {
                                "PASS" => {},
                                "FAIL" => {
                                    fail_count += 1;
                                },
                                _ => {
                                    parse_state = PARSESTATES::START;
                                    if fail_count>0{
                                        slice_start = x + 1;
                                    }
                                }
                            }

                        },

                    }
            }

            if slice_start<df.height(){
                //println!("pushing slice end: {} {}",slice_start,df.height()-slice_start);
                slice_rows.push((slice_start,df.height()-slice_start));
            }

            let mut base_df = df.slice(0,0);
            for (_x,(start,len)) in slice_rows.iter().enumerate(){
                if len>&0{
                    let slice_df = df.slice(*start as i64,*len);
                    base_df = base_df.vstack(&slice_df).unwrap();
                }
            }
            //println!("Filtered DataFrame: {}",&base_df);
            return Result::Ok(base_df);
   
        }

    }

    Err(PolarsError::NoData("No file to filter".into()))
}

pub fn save_merged(df:&mut DataFrame,path:Option<PathBuf>){


    if let Some(path) = path{

        write_csv(df,path);
    }
}


//use polars_excel_writer::ExcelWriter;

//pub fn write_xlsx(_df:DataFrame, _path:PathBuf){
//    let mut output_file = File::create(path).unwrap(); 
//    let result = ExcelWriter::new(&mut output_file)
//    .finish(&df);

//}

pub fn write_csv(df:&mut DataFrame, path:PathBuf){
    let mut output_file = File::create(path).unwrap(); 
    let _result = CsvWriter::new(&mut output_file)
    .finish(df);

}
