use egui::TextStyle;

use crate::clean_df_val;


/// Shows off a table with dynamic layout
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct Table {
    striped: bool,
    resizable: bool,
    clickable: bool,


    scroll_to_row: Option<usize>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            striped: true,
            resizable: true,
            clickable: true,
            scroll_to_row: None,
        }
    }
}


impl Table {
    pub fn ui(&mut self, ui: &mut egui::Ui,df: &polars::prelude::DataFrame) {


        ui.separator();

        let body_text_size = TextStyle::Body.resolve(ui.style()).size;
        use egui_extras::{Size, StripBuilder};
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(100.0)) // for the table
            .size(Size::exact(body_text_size)) // for the source code link
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                            self.table_ui(ui,df);
                    });
                });
            });
    }
}

impl Table {
    fn table_ui(&mut self, ui: &mut egui::Ui,df:&polars::prelude::DataFrame) {
        use egui_extras::{Column, TableBuilder};

        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

            let mut table= TableBuilder::new(ui)
            .striped(self.striped)
            .resizable(self.resizable)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);

        for _n in 0..df.width()+1{ //+1 to allow for row number column
            table = table.column(Column::initial(100.0).at_least(40.0).clip(true))
            //tbl = tbl.column(Column::initial(100.0).range(40.0..=300.0))
        }
        table = table.column(Column::remainder());



        if self.clickable {
            table = table.sense(egui::Sense::click());
        }

        if let Some(row_index) = self.scroll_to_row.take() {
            table = table.scroll_to_row(row_index, None);
        }

        table

            .body(|body| 



                body.rows(text_height, df.height(), |mut row| {

                    let row_index = row.index();

                    let columns = df.columns(df.get_column_names()).unwrap();
                    for column in columns{
                        
                        if let Ok(df_val) = column.get(row_index){
                            row.col(|ui| {
                                let df_val = clean_df_val(df_val.clone());
                                    ui.label(format!("{}",df_val));

                                });
                            }
                        }
                    }));
    }

}
//    }

//}

