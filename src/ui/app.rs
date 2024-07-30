use ratatui::{buffer::Buffer, layout::{Alignment, Constraint, Layout, Rect}, style::Stylize, widgets::{Block, Borders, Paragraph, Widget}};

use crate::net::NetClient;


struct App {
    net: NetClient
}

impl App {
    fn new() -> Self {
        App {
            net: NetClient::new(),
        }
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Geohog")
            .bold()
            .centered()
            .render(area, buf);
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        let outer_block = Block::new()
            .borders(Borders::NONE)
            .title_alignment(Alignment::Left)
            .title("Connections");
        let inner_block = Block::new()
            .borders(Borders::NONE);

        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        outer_block.render(outer_area, buf);
    }   

    fn render_info(&self, area: Rect, buf: &mut Buffer) {

    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized 
    {
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
        ]);
        let [header_area, main_area] = vertical.areas(area);
        
        let horizontal = Layout::horizontal([
            Constraint::Percentage(70),
            Constraint::Percentage(30)
        ]);
        let [list_area, info_area] = horizontal.areas(main_area);
        self.render_header(header_area, buf);
        self.render_list(list_area, buf);
        self.render_info(info_area, buf);
    }
}