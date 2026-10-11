pub mod components;
pub mod connect;
pub mod data_panel;
pub mod forms;
pub mod main_view;
pub mod popups;

use crate::app::{App, Screen};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Connect => connect::draw_connect(f, app),
        Screen::Databases => main_view::draw_databases(f, app),
        Screen::Schemas => main_view::draw_schemas(f, app),
        Screen::Main => main_view::draw_main(f, app),
        Screen::Query => {
            main_view::draw_main(f, app);
            popups::draw_query_popup(f, app);
        }
        Screen::QueryHistory => {
            main_view::draw_main(f, app);
            popups::draw_query_history_popup(f, app);
        }
        Screen::Search => {
            main_view::draw_main(f, app);
            popups::draw_search_bar(f, app);
        }
        Screen::CrudForm => forms::draw_crud_form(f, app),
        Screen::ConfirmDelete => {
            main_view::draw_main(f, app);
            forms::draw_confirm_delete(f, app);
        }
        Screen::Help => {
            main_view::draw_main(f, app);
            popups::draw_help_popup(f);
        }
        Screen::TableStats => {
            main_view::draw_main(f, app);
            popups::draw_table_stats_popup(f, app);
        }
        Screen::Jump => {
            main_view::draw_main(f, app);
            popups::draw_jump_bar(f, app);
        }
        Screen::MultilineEditor => {
            forms::draw_crud_form(f, app);
            forms::draw_multiline_editor(f, app);
        }
        Screen::ExportForm => {
            main_view::draw_main(f, app);
            forms::draw_export_popup(f, app);
        }
        Screen::ImportForm => {
            main_view::draw_main(f, app);
            forms::draw_import_popup(f, app);
        }
        Screen::SaveQueryPrompt => {
            main_view::draw_main(f, app);
            popups::draw_save_query_prompt(f, app);
        }
        Screen::NamedQueriesList => {
            main_view::draw_main(f, app);
            popups::draw_named_queries_list(f, app);
        }
    }
}
