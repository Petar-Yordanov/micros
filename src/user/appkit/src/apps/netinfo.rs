extern crate alloc;

use alloc::string::String;
use core::fmt::Write;

use libui::canvas::Canvas;
use libui::color::{PANEL_TEXT, TEXT, TEXT_DIM};
use libui::event::{CursorKind, MouseButton, UiEvent};
use libui::geom::{Point, Rect};
use libui::text::draw_text;
use libui::widgets::button::draw_button;
use libui::widgets::panel::{draw_panel, inner_rect};

use micros_abi::types::{
    NetInfo, ABI_NET_INFO_F_HAS_IPV4, ABI_NET_INFO_F_HAS_MAC, ABI_NET_INFO_F_LINK_UP,
};

use crate::app::App;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolbarButton {
    None,
    Refresh,
    PingGateway,
    HttpGet,
}

pub struct NetInfoApp {
    info: Option<NetInfo>,
    errno: Option<i32>,
    hover_button: ToolbarButton,
    pressed_button: ToolbarButton,
    status: String,
    http_preview: String,
}

impl NetInfoApp {
    pub fn new() -> Self {
        let mut this = Self {
            info: None,
            errno: None,
            hover_button: ToolbarButton::None,
            pressed_button: ToolbarButton::None,
            status: String::from("Loading network information..."),
            http_preview: String::from("Click HTTP GET to fetch http://10.0.2.2/"),
        };

        this.refresh();
        this
    }

    fn toolbar_rect_local() -> Rect {
        Rect::new(8, 8, 420, 28)
    }

    fn refresh_button_rect_local() -> Rect {
        Rect::new(12, 12, 78, 20)
    }

    fn ping_button_rect_local() -> Rect {
        Rect::new(96, 12, 86, 20)
    }

    fn http_button_rect_local() -> Rect {
        Rect::new(188, 12, 86, 20)
    }

    fn summary_rect_local() -> Rect {
        Rect::new(8, 44, 420, 128)
    }

    fn details_rect_local() -> Rect {
        Rect::new(8, 180, 420, 88)
    }

    fn http_rect_local() -> Rect {
        Rect::new(8, 276, 420, 112)
    }

    fn status_rect_local() -> Rect {
        Rect::new(8, 396, 420, 28)
    }

    fn button_at(pos: Point) -> ToolbarButton {
        if Self::refresh_button_rect_local().contains(pos) {
            ToolbarButton::Refresh
        } else if Self::ping_button_rect_local().contains(pos) {
            ToolbarButton::PingGateway
        } else if Self::http_button_rect_local().contains(pos) {
            ToolbarButton::HttpGet
        } else {
            ToolbarButton::None
        }
    }

    fn refresh(&mut self) {
        match rlibc::net::info() {
            Ok(info) => {
                self.info = Some(info);
                self.errno = None;

                self.status.clear();
                self.status.push_str("Network information refreshed");
            }
            Err(e) => {
                self.info = None;
                self.errno = Some(e.0);

                self.status.clear();
                self.status.push_str("Failed to query network information");
            }
        }
    }

    fn ping_gateway(&mut self) {
        match rlibc::net::ping_gateway() {
            Ok(rlibc::net::PingGatewayResult::EchoRequestSent) => {
                self.status.clear();
                self.status.push_str("ICMP echo request sent to gateway");
            }
            Ok(rlibc::net::PingGatewayResult::ArpRequestSent) => {
                self.status.clear();
                self.status
                    .push_str("ARP request sent for gateway; click Ping GW again");
            }
            Err(e) => {
                self.status.clear();
                let _ = write!(&mut self.status, "Ping gateway failed with errno {}", e.0);
            }
        }
    }

    fn http_get(&mut self) {
        self.status.clear();
        self.status.push_str("Fetching http://httpforever.com/...");

        self.http_preview.clear();

        match rlibc::http::get_url("http://httpforever.com/") {
            Ok(bytes) => {
                let _ = write!(
                    &mut self.status,
                    "HTTP GET complete: {} bytes received",
                    bytes.len()
                );

                if bytes.is_empty() {
                    self.http_preview.push_str("(empty response)");
                    return;
                }

                let preview_len = core::cmp::min(bytes.len(), 360);

                for &b in &bytes[..preview_len] {
                    match b {
                        b'\r' => {}
                        b'\n' => self.http_preview.push('\n'),
                        0x20..=0x7e => self.http_preview.push(b as char),
                        b'\t' => self.http_preview.push(' '),
                        _ => self.http_preview.push('.'),
                    }
                }

                if bytes.len() > preview_len {
                    self.http_preview.push_str("\n...");
                }
            }
            Err(e) => {
                let _ = write!(&mut self.status, "HTTP GET failed with errno {}", e.0);
                let _ = write!(
                    &mut self.http_preview,
                    "Failed to fetch http://httpforever.com/\nerrno={}\n\nDNS uses QEMU user-network DNS server 10.0.2.3.",
                    e.0
                );
            }
        }
    }

    fn has_flag(info: NetInfo, flag: u32) -> bool {
        (info.flags & flag) != 0
    }

    fn bool_text(value: bool) -> &'static str {
        if value {
            "yes"
        } else {
            "no"
        }
    }

    fn link_text(info: NetInfo) -> &'static str {
        if Self::has_flag(info, ABI_NET_INFO_F_LINK_UP) {
            "up"
        } else {
            "down"
        }
    }

    fn write_mac(out: &mut String, mac: [u8; 6]) {
        let _ = write!(
            out,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        );
    }

    fn write_ipv4(out: &mut String, ip: [u8; 4]) {
        let _ = write!(out, "{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
    }

    fn draw_line(canvas: &mut Canvas, x: i32, y: i32, label: &str, value: &str) {
        draw_text(canvas, x, y, TEXT_DIM, None, label);
        draw_text(canvas, x + 104, y, PANEL_TEXT, None, value);
    }

    fn draw_multiline(
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        color: u32,
        text: &str,
        max_lines: usize,
    ) {
        let bytes = text.as_bytes();
        let mut start = 0usize;
        let mut line = 0usize;

        while start < bytes.len() && line < max_lines {
            let mut end = start;

            while end < bytes.len() && bytes[end] != b'\n' {
                end += 1;
            }

            if let Ok(s) = core::str::from_utf8(&bytes[start..end]) {
                draw_text(canvas, x, y + (line as i32 * 14), color, None, s);
            }

            line += 1;
            start = if end < bytes.len() { end + 1 } else { end };
        }
    }
}

impl App for NetInfoApp {
    fn title(&self) -> &'static str {
        "NET INFO"
    }

    fn cursor(&self, local_pos: Point) -> CursorKind {
        if Self::button_at(local_pos) != ToolbarButton::None {
            CursorKind::Hand
        } else {
            CursorKind::Arrow
        }
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let mut changed = false;

        match *ev {
            UiEvent::MouseMove { pos } => {
                let hover = Self::button_at(pos);
                if hover != self.hover_button {
                    self.hover_button = hover;
                    changed = true;
                }
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                let btn = Self::button_at(pos);
                if btn != ToolbarButton::None && self.pressed_button != btn {
                    self.pressed_button = btn;
                    changed = true;
                }
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                let released_over = Self::button_at(pos);
                let pressed = self.pressed_button;

                if self.pressed_button != ToolbarButton::None {
                    self.pressed_button = ToolbarButton::None;
                    changed = true;
                }

                if pressed != ToolbarButton::None && pressed == released_over {
                    match pressed {
                        ToolbarButton::Refresh => self.refresh(),
                        ToolbarButton::PingGateway => self.ping_gateway(),
                        ToolbarButton::HttpGet => self.http_get(),
                        ToolbarButton::None => {}
                    }

                    changed = true;
                }
            }

            UiEvent::KeyDown { code } => {
                if code == 63 {
                    self.refresh();
                    changed = true;
                }
            }

            UiEvent::KeyUp { .. } => {}
            UiEvent::MouseWheel { .. } => {}
            UiEvent::MouseDown { .. } => {}
            UiEvent::MouseUp { .. } => {}
        }

        changed
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        let toolbar = Self::toolbar_rect_local().translate(client_rect.x, client_rect.y);
        let refresh_btn = Self::refresh_button_rect_local().translate(client_rect.x, client_rect.y);
        let ping_btn = Self::ping_button_rect_local().translate(client_rect.x, client_rect.y);
        let http_btn = Self::http_button_rect_local().translate(client_rect.x, client_rect.y);
        let summary = Self::summary_rect_local().translate(client_rect.x, client_rect.y);
        let details = Self::details_rect_local().translate(client_rect.x, client_rect.y);
        let http = Self::http_rect_local().translate(client_rect.x, client_rect.y);
        let status = Self::status_rect_local().translate(client_rect.x, client_rect.y);

        draw_panel(canvas, toolbar);

        draw_button(
            canvas,
            refresh_btn,
            "Refresh",
            self.hover_button == ToolbarButton::Refresh,
            self.pressed_button == ToolbarButton::Refresh,
        );

        draw_button(
            canvas,
            ping_btn,
            "Ping GW",
            self.hover_button == ToolbarButton::PingGateway,
            self.pressed_button == ToolbarButton::PingGateway,
        );

        draw_button(
            canvas,
            http_btn,
            "HTTP GET",
            self.hover_button == ToolbarButton::HttpGet,
            self.pressed_button == ToolbarButton::HttpGet,
        );

        draw_text(
            canvas,
            toolbar.x + 286,
            toolbar.y + 10,
            TEXT_DIM,
            None,
            "F5 refreshes",
        );

        draw_panel(canvas, summary);
        draw_text(
            canvas,
            summary.x + 8,
            summary.y - 12,
            TEXT_DIM,
            None,
            "Network adapter",
        );

        let summary_inner = inner_rect(summary, 8);

        match self.info {
            Some(info) => {
                let mut flags_line = String::new();
                let _ = write!(
                    &mut flags_line,
                    "link={} mac={} ipv4={}",
                    Self::bool_text(Self::has_flag(info, ABI_NET_INFO_F_LINK_UP)),
                    Self::bool_text(Self::has_flag(info, ABI_NET_INFO_F_HAS_MAC)),
                    Self::bool_text(Self::has_flag(info, ABI_NET_INFO_F_HAS_IPV4)),
                );

                let mut mtu_line = String::new();
                let _ = write!(&mut mtu_line, "{}", info.mtu);

                let mut mac_line = String::new();
                if Self::has_flag(info, ABI_NET_INFO_F_HAS_MAC) {
                    Self::write_mac(&mut mac_line, info.mac);
                } else {
                    mac_line.push_str("(not available)");
                }

                Self::draw_line(
                    canvas,
                    summary_inner.x,
                    summary_inner.y + 4,
                    "Link:",
                    Self::link_text(info),
                );
                Self::draw_line(
                    canvas,
                    summary_inner.x,
                    summary_inner.y + 20,
                    "MTU:",
                    &mtu_line,
                );
                Self::draw_line(
                    canvas,
                    summary_inner.x,
                    summary_inner.y + 36,
                    "MAC:",
                    &mac_line,
                );
                Self::draw_line(
                    canvas,
                    summary_inner.x,
                    summary_inner.y + 52,
                    "Flags:",
                    &flags_line,
                );
            }
            None => {
                let mut err = String::new();
                match self.errno {
                    Some(errno) => {
                        let _ = write!(&mut err, "net::info failed with errno {}", errno);
                    }
                    None => {
                        err.push_str("No network information available");
                    }
                }

                draw_text(
                    canvas,
                    summary_inner.x,
                    summary_inner.y + 4,
                    TEXT,
                    None,
                    &err,
                );
            }
        }

        draw_panel(canvas, details);
        draw_text(
            canvas,
            details.x + 8,
            details.y - 12,
            TEXT_DIM,
            None,
            "IPv4 configuration",
        );

        let details_inner = inner_rect(details, 8);

        match self.info {
            Some(info) if Self::has_flag(info, ABI_NET_INFO_F_HAS_IPV4) => {
                let mut ip = String::new();
                let mut mask = String::new();
                let mut gateway = String::new();

                Self::write_ipv4(&mut ip, info.ipv4);
                Self::write_ipv4(&mut mask, info.netmask);
                Self::write_ipv4(&mut gateway, info.gateway);

                Self::draw_line(canvas, details_inner.x, details_inner.y + 4, "IP:", &ip);
                Self::draw_line(
                    canvas,
                    details_inner.x,
                    details_inner.y + 20,
                    "Netmask:",
                    &mask,
                );
                Self::draw_line(
                    canvas,
                    details_inner.x,
                    details_inner.y + 36,
                    "Gateway:",
                    &gateway,
                );
            }
            Some(_) => {
                draw_text(
                    canvas,
                    details_inner.x,
                    details_inner.y + 4,
                    TEXT,
                    None,
                    "IPv4 is not configured",
                );
            }
            None => {
                draw_text(
                    canvas,
                    details_inner.x,
                    details_inner.y + 4,
                    TEXT,
                    None,
                    "No IPv4 information available",
                );
            }
        }

        draw_panel(canvas, http);
        draw_text(
            canvas,
            http.x + 8,
            http.y - 12,
            TEXT_DIM,
            None,
            "HTTP response preview",
        );

        let http_inner = inner_rect(http, 8);
        Self::draw_multiline(
            canvas,
            http_inner.x,
            http_inner.y + 4,
            PANEL_TEXT,
            &self.http_preview,
            7,
        );

        draw_panel(canvas, status);
        let status_inner = inner_rect(status, 6);

        draw_text(
            canvas,
            status_inner.x,
            status_inner.y + 5,
            TEXT,
            None,
            &self.status,
        );
    }
}
