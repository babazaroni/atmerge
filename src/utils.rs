use polars::frame::row;
use polars_core::prelude::*;

use polars_core::utils::rayon::result;
use polars_io::prelude::*;

use std::hash::Hash;
use std::io::prelude::*;
use std::str::FromStr;
use std::{any, fs, i64};

use std::fs::File;
use std::io::{self, BufRead, LineWriter};
use std::path::{Path,PathBuf};
use std::collections::{HashMap, HashSet};

use itertools::izip;




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

pub fn get_format_file(path:&PathBuf) -> Option<PathBuf>{

        let entries = std::fs::read_dir(path)
        .unwrap();

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file(){
                    let path_str = path.to_str().unwrap().to_ascii_lowercase();
                    if path_str.contains("format"){
                        return Some(path.clone());
                    }
                }
            }
        }
    None
}

pub fn get_paths_from_part_folder(path: &Option<PathBuf>) -> (Option<std::path::PathBuf>,Option<std::path::PathBuf>,Option<std::path::PathBuf>){

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

pub fn save_csv(df:&mut DataFrame,path:Option<PathBuf>){
    if let Some(picked_path) = path {
        let mut output_file = File::create(picked_path).unwrap(); 
        let _result = CsvWriter::new(&mut output_file)
        .with_quote_style(QuoteStyle::NonNumeric)
        .has_header(false)
        .finish(df);
    }
}

pub fn fix_quotes(pb: &PathBuf){
    let mut new_lines = Vec::new();
    if let Ok(lines) = read_lines(pb.to_str().unwrap()) {
        for line in lines.flatten() {
            let mut new_line = line.clone();
            new_line = new_line.replace("\"","");
            new_lines.push(new_line);
        }
    }
    let file = File::create(pb.to_str().unwrap()).unwrap();
    let mut file = LineWriter::new(file);
    for line in new_lines{
        file.write_all(line.as_bytes()).unwrap();
        file.write_all(b"\n").unwrap();
    }
}


pub fn prompt_for_csv() -> PolarsResult<DataFrame> {

    let path = rfd::FileDialog::new()
    .set_directory(".")
    //.add_filter("CSV",&["csv"]).pick_file();
    .pick_folder();

    load_csv(path)

}

pub fn get_files_with_extension(dir:&PathBuf,desired_ext:&str) -> Result<Vec<PathBuf>, Box<dyn ::std::error::Error>>{
    let paths = std::fs::read_dir(dir)?
        // Filter out all those directory entries which couldn't be read
        .filter_map(|res| res.ok())
        // Map the directory entries to paths
        .map(|dir_entry| dir_entry.path())
        // Filter out all paths with extensions other than `csv`
        .filter_map(|path| {
            if path.extension().map_or(false, |ext| ext == desired_ext) {Some(path)
            } else {None}
        })
        .collect::<Vec<_>>();

    Ok(paths)

}

pub fn compare_with_trailing_number(a: &str, b: &str) -> std::cmp::Ordering{
    let (a_prefix, a_number) = split_by_trailing_number(a);
    let (b_prefix, b_number) = split_by_trailing_number(b);

    if a_prefix == b_prefix {
        return a_number.cmp(&b_number);
    }

    a_prefix.cmp(&b_prefix)
}


pub fn split_by_trailing_number(s: &str) -> (String, i32) {

    let a = s.split('.').collect::<Vec<&str>>();

    let first = a[..a.len()-1].join(".");

    let reversed_number = first.chars().rev().take_while(|c| c.is_numeric()).collect::<String>();

    let trailing_number_string = reversed_number.chars().rev().collect::<String>();

    let prefix = first.chars().rev().skip(trailing_number_string.len()).collect::<String>();

    if trailing_number_string.len() == 0{
        return (prefix,0);
    }


    (prefix, trailing_number_string.parse::<i32>().unwrap())
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


// Remember that xslx files with not all columns filled in can cause a crashg
pub fn get_df_from_xlsx(path:Option<PathBuf>) -> PolarsResult<DataFrame> {

    //println!("get_df_from_xlsx path1: {:?}",path);

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

        //println!("get_df_from_xlsx path2: {:?}",picked_path.display());
        
     //   let mut workbook: Xlsx<_> = open_workbook(&picked_path).expect("Cannot open file"); 
         let workbook = open_workbook(&picked_path);

         if workbook.is_err(){
            return Err(PolarsError::NoData("Cannot open file".into()));}

        let mut workbook: Xlsx<_> = workbook.unwrap();

        let res = workbook.load_tables();

        if res.is_err(){
            return Err(PolarsError::NoData("Cannot load tables".into()));
        }
        
        let sheets = workbook.sheet_names().to_owned();
        let first_sheet = sheets[0].clone();

        //println!("get_df_from_xlsx path: {:?}",picked_path.display());

        //println!("get_df_from_xlsx Found {} sheets in '{}'",sheets.len(), first_sheet);

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

fn read_lines(filename: &str) -> io::Result<io::Lines<io::BufReader<File>>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
#[derive(Debug)]
pub struct ReportFormat {
    start_row: usize,
    test_delim: String,
    source_columns: HashSet<String>,
    test_sources: HashMap<String,String>,
    test_dests: HashMap<String,String>,
}
impl ReportFormat {
    pub fn new(format_file: &PathBuf) -> ReportFormat {

        let mut test_delim = String::from("RUN TIME");
        let mut start_row = 0;
        let mut source_columns = HashSet::new();

        let mut test_counts = HashMap::new();
        let mut test_sources = HashMap::new();
        let mut test_dests = HashMap::new();

        if let Ok(lines) = read_lines(format_file.to_str().unwrap()) {


    
            for line in lines.flatten() {
                if line.contains("#"){continue;}
    
                let parts:Vec<String>= line.split(",").map(String::from).collect();
    
        
                match parts[0].as_ref(){
                    "START_ROW" => {
                        if parts.len()>1{
                            start_row = parts[1].parse::<usize>().unwrap();
                            //println!("merge_excel_format: start_row: {}",start_row);
                        }
                    },
                    "TEST" => {
                        if parts.len()>3{
                            let mut test_name = parts[1].clone();
                            let test_source = parts[2].clone();
                            let test_dest = parts[3].clone();
    
                            source_columns.insert(test_source.clone());
                            let count = test_counts.entry(test_name.clone()).or_insert(0);
                            *count += 1;
    
                            test_name = format!("{}-{}",test_name,count);
    
    
                            //println!("test_name insert source: {} {}",test_name,test_source);
                            test_sources.insert(test_name.clone(),test_source);
    
                            //println!("test_name insert dest: {} {}",test_name,test_dest);
                            test_dests.insert(test_name,test_dest);
    
    
                        }
                    },
                    "UNIT_NUM" => {
                        //println!("found UNIT#");
                        if parts.len()>1{
                            let test_name = parts[0].clone();
                            let test_dest = parts[1].clone();
                            test_dests.insert(test_name,test_dest);
                        }
                    },
                    "TEST_DELIM" => {
                        if parts.len()>1{
                            test_delim = parts[1].clone();
                        }
                    },
                    _ => {}
                }
            }
        }


        ReportFormat {
            start_row: start_row,
            test_delim: test_delim,
            source_columns: source_columns,
            test_sources: test_sources,
            test_dests: test_dests,
        }
    }
}

pub fn merge_excel_format(df_tests:&DataFrame,source_path: PathBuf,dest_path: &PathBuf,report_format:&ReportFormat){


    let mut book = umya_spreadsheet::reader::xlsx::read(source_path).unwrap();


    let mut row_test_results: HashMap<String, String> = HashMap::new();
    let mut results_count: HashMap<String, i32> = HashMap::new();


    for source_column in report_format.source_columns.iter(){

        let mut unit_num = 1;
        let mut current_row = report_format.start_row;

        //println!("processing source_column: {}",source_column);
        //println!("");

        let columns = get_vec_columns(&df_tests);

        //println!("columns: {:?}",columns);

        let test_column = get_column_with_header("TEST",columns.clone());
        let passfail_column = get_column_with_header("PASS/FAIL",columns);


        if let Some(test_column) = test_column{

    
            let columns = get_vec_columns(&df_tests);
            let result_column  = get_column_with_header(source_column,columns);
            if let Some(result_column) = result_column{


                if let Some(passfail_column) = passfail_column{


                    for ((test_val,result_val),passfail_val) in test_column.iter().zip(result_column.iter()).zip(passfail_column.iter()){
                    //for (test_val,x,result_val) in izip!(test_column.iter(),result_column.iter(),passfail_column.iter()){

                        row_test_results.insert(String::from("UNIT_NUM"),String::from(format!("{}",unit_num)));

                        let mut tval = match test_val{
                            polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                            _ => ""
                        };
                        let rval = match result_val{
                            polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                            _ => ""
                        };

                        if tval == "" {continue;}

                        tval = tval.trim(); // remove leading/trailing whitespace

                        if tval == report_format.test_delim{

//                            let pfval = match passfail_val{
//                                polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
//                                _ => ""
//                            };


//                            if pfval == "PASS"{

                            //println!("row_test_results: {:?}",row_test_results);

                                for (key,value) in row_test_results.iter(){
                                    let column = report_format.test_dests.get(key).unwrap();
                                    let header = format!("{}{}",column,current_row);
                                    let _ = book
                                        .get_sheet_mut(&0)
                                        .unwrap()
                                        .get_cell_mut(header)
                                        .set_value(value);

                                }
                                row_test_results = HashMap::new();
                                results_count = HashMap::new();

                                current_row += 1;
                                unit_num += 1;
                                continue;
//                            }
                        }

                        let count = results_count.entry(tval.to_string().clone()).or_insert(0);
                        *count += 1;

                        let test_name = format!("{}-{}",tval,count);

                        //println!("checking test_name: {} result: {}",test_name,rval);

                        if report_format.test_sources.get(&test_name) == None{
                            continue;
                        } 


                        //println!("checking test sources {} {:?}",test_name,test_sources);
                        // is this test in the current source column?
                        if source_column != report_format.test_sources.get(&test_name).unwrap(){continue}


                        //println!("test_name: {} result: {}",test_name,rval);

                        tval = &test_name;


                        match row_test_results.get(tval){
                            Some(results) => {
                                let mut mresults = results.to_owned();
                                mresults = format!("{},{}",mresults,rval);
                            
                                row_test_results.insert(tval.to_string(),mresults);
                            },


                            None => {
                                match report_format.test_dests.get(tval){
                                    Some(_column) => {
                                        row_test_results.insert(tval.to_string(),rval.to_string());
                                    },
                                    None => {}
                                }
                            }
                        }
                    }
                }

            }

        }

        //break;

    }

    //println!("dest_path: {:?}",dest_path);

    let _ = umya_spreadsheet::writer::xlsx::write(&mut book, dest_path);


}
pub fn merge_excel_append(df_template:&DataFrame,df_tests:&DataFrame,source_path: PathBuf,dest_path: &PathBuf){

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

    println!("merge_excel_append: test_row: {}",test_row.unwrap());

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

    excel_fix_id_column(&mut book,test_row.unwrap());

    let _ = umya_spreadsheet::writer::xlsx::write(&mut book, dest_path);


}

enum PARSESTATES {
    START,
    END
}

fn excel_fix_id_column(book: &mut umya_spreadsheet::Spreadsheet,test_row:usize){
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

fn get_column_with_header(header:&str,columns:Vec<Series>) -> Option<Series>{

    for column in columns{
        let column = column.rechunk();  // do this or iterating will crash

        for entry in column.iter(){

            let val = match entry{
                polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                _ => "_"

            };

            if val == header{
                //println!("Found PASS/FAIL column");
                return Some(column.clone());
            }

        }
    }
    None
}
#[derive(Debug)]
enum FILTERSTATES {
    FIND_FIRST_TEST,
    REST

}
pub fn filter_fails(df:Option<DataFrame>,report_format:&ReportFormat) -> (PolarsResult<DataFrame>,usize) {


    if let Some(df) = df {
        let columns = get_vec_columns(&df);
        let pf_column = get_column_with_header("PASS/FAIL",columns);
        let columns = get_vec_columns(&df);
        let test_column = get_column_with_header("TEST",columns);
        let mut slice_rows: Vec<(usize,usize)> = Vec::new();

        let mut parse_state = FILTERSTATES::FIND_FIRST_TEST;

        let mut test_start: Option<usize> = None;

        if let Some(test_column) = test_column{


            if let Some(pf_column) = pf_column{

                let mut fail_count = 0;


                for (x,(test_val,passfail_val)) in test_column.iter().zip(pf_column.iter()).enumerate(){

                    let tval = match test_val{
                        polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                        _ => "_"
                    };
                    let pfval = match passfail_val{
                        polars::datatypes::AnyValue::Utf8(val) => val.trim_matches('"'),
                        _ => "_"
                    };
                    
                    match parse_state{
                        FILTERSTATES::FIND_FIRST_TEST => {

                            if tval == "TEST"{
                                slice_rows.push((0,x+1));
                                parse_state = FILTERSTATES::REST;
                            }
                        },
                        FILTERSTATES::REST => {
                            if test_start == None{
                                test_start = Some(x);
                            }
                            if pfval == "FAIL"{
                                fail_count += 1;
                            }

                            if tval == report_format.test_delim{
                                if fail_count == 0{
                                    slice_rows.push((test_start.unwrap(),x - test_start.unwrap() + 1));
                                }

                                test_start = None;
                                fail_count = 0;
                            }

                        }
                    }
                }
            }
        }

        let mut base_df = df.slice(0,0);
        for (_x,(start,len)) in slice_rows.iter().enumerate(){
            if len>&0{
                let slice_df = df.slice(*start as i64,*len);
                base_df = base_df.vstack(&slice_df).unwrap();
            }
        }
        //println!("Filtered DataFrame: {}",&base_df);
        return (Result::Ok(base_df),slice_rows.len()-1);

    }

    (Err(PolarsError::NoData("No file to filter".into())),0)
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
