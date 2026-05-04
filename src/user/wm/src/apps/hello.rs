use core::fmt::Write;

use libui::canvas::Canvas;
use libui::color::TEXT;
use libui::event::{MouseButton, UiEvent};
use libui::geom::Rect;
use libui::text::draw_text;
use libui::widgets::button::draw_button;
use micros_abi::errno;
use micros_abi::types::{NetInfo, ABI_NET_INFO_F_HAS_MAC, ABI_NET_INFO_F_LINK_UP};
use rlibc::net;

use crate::app::App;

const TEST_SRC_IP: [u8; 4] = [10, 0, 2, 15];
const TEST_TARGET_IP: [u8; 4] = [10, 0, 2, 2];

#[derive(Clone, Copy)]
enum TxState {
    Idle,
    Ok(usize),
    Err(i32),
}

#[derive(Clone, Copy)]
enum RxDetail {
    None,
    Arp {
        oper: u16,
        sha: [u8; 6],
        spa: [u8; 4],
        tha: [u8; 6],
        tpa: [u8; 4],
    },
    Ipv4 {
        proto: u8,
        src: [u8; 4],
        dst: [u8; 4],
    },
}

#[derive(Clone, Copy)]
enum RxState {
    Idle,
    NoFrame,
    Err(i32),
    Frame {
        len: usize,
        dst_mac: [u8; 6],
        src_mac: [u8; 6],
        ethertype: u16,
        detail: RxDetail,
    },
}

pub struct HelloApp {
    count: u32,

    hovered_click: bool,
    pressed_click: bool,

    hovered_info: bool,
    pressed_info: bool,

    hovered_send: bool,
    pressed_send: bool,

    hovered_recv: bool,
    pressed_recv: bool,

    nic_queried: bool,
    nic_info_err: Option<i32>,
    nic_present: bool,
    nic_link_up: bool,
    nic_has_mac: bool,
    nic_mtu: u32,
    nic_mac: [u8; 6],

    tx_state: TxState,
    rx_state: RxState,
}

impl HelloApp {
    pub fn new() -> Self {
        Self {
            count: 0,

            hovered_click: false,
            pressed_click: false,

            hovered_info: false,
            pressed_info: false,

            hovered_send: false,
            pressed_send: false,

            hovered_recv: false,
            pressed_recv: false,

            nic_queried: false,
            nic_info_err: None,
            nic_present: false,
            nic_link_up: false,
            nic_has_mac: false,
            nic_mtu: 0,
            nic_mac: [0; 6],

            tx_state: TxState::Idle,
            rx_state: RxState::Idle,
        }
    }

    fn click_button_rect_local() -> Rect {
        Rect::new(16, 48, 120, 32)
    }

    fn info_button_rect_local() -> Rect {
        Rect::new(16, 88, 120, 32)
    }

    fn send_button_rect_local() -> Rect {
        Rect::new(16, 128, 120, 32)
    }

    fn recv_button_rect_local() -> Rect {
        Rect::new(16, 168, 120, 32)
    }

    fn refresh_nic_info(&mut self) {
        self.nic_queried = true;

        match net::info() {
            Ok(info) => self.apply_net_info(info),
            Err(e) => {
                self.nic_info_err = Some(e.0);
                self.nic_present = false;
                self.nic_link_up = false;
                self.nic_has_mac = false;
                self.nic_mtu = 0;
                self.nic_mac = [0; 6];
            }
        }
    }

    fn apply_net_info(&mut self, info: NetInfo) {
        self.nic_info_err = None;
        self.nic_present = true;
        self.nic_link_up = (info.flags & ABI_NET_INFO_F_LINK_UP) != 0;
        self.nic_has_mac = (info.flags & ABI_NET_INFO_F_HAS_MAC) != 0;
        self.nic_mtu = info.mtu;
        self.nic_mac = info.mac;
    }

    fn send_arp_probe(&mut self) {
        self.refresh_nic_info();

        if !self.nic_present {
            self.tx_state = TxState::Err(errno::ENODEV as i32);
            return;
        }

        if !self.nic_has_mac {
            self.tx_state = TxState::Err(errno::ENODEV as i32);
            return;
        }

        let mut frame = [0u8; 42];

        frame[0..6].copy_from_slice(&[0xff; 6]);
        frame[6..12].copy_from_slice(&self.nic_mac);
        frame[12] = 0x08;
        frame[13] = 0x06;

        frame[14] = 0x00;
        frame[15] = 0x01;

        frame[16] = 0x08;
        frame[17] = 0x00;

        frame[18] = 6;
        frame[19] = 4;

        frame[20] = 0x00;
        frame[21] = 0x01;

        frame[22..28].copy_from_slice(&self.nic_mac);
        frame[28..32].copy_from_slice(&TEST_SRC_IP);
        frame[32..38].copy_from_slice(&[0u8; 6]);
        frame[38..42].copy_from_slice(&TEST_TARGET_IP);

        match net::send(&frame) {
            Ok(n) => self.tx_state = TxState::Ok(n),
            Err(e) => self.tx_state = TxState::Err(e.0),
        }
    }

    fn poll_rx(&mut self) {
        let mut buf = [0u8; 1600];

        match net::recv(&mut buf) {
            Ok(0) => {
                self.rx_state = RxState::NoFrame;
            }
            Ok(n) => {
                self.rx_state = Self::parse_rx_frame(&buf, n);
            }
            Err(e) => {
                if e.0 == errno::EAGAIN as i32 {
                    self.rx_state = RxState::NoFrame;
                } else {
                    self.rx_state = RxState::Err(e.0);
                }
            }
        }
    }

    fn parse_rx_frame(buf: &[u8], len: usize) -> RxState {
        if len < 14 {
            return RxState::Frame {
                len,
                dst_mac: [0; 6],
                src_mac: [0; 6],
                ethertype: 0,
                detail: RxDetail::None,
            };
        }

        let mut dst_mac = [0u8; 6];
        let mut src_mac = [0u8; 6];
        dst_mac.copy_from_slice(&buf[0..6]);
        src_mac.copy_from_slice(&buf[6..12]);

        let ethertype = u16::from_be_bytes([buf[12], buf[13]]);

        let detail = if ethertype == 0x0806 && len >= 42 {
            let oper = u16::from_be_bytes([buf[20], buf[21]]);

            let mut sha = [0u8; 6];
            let mut spa = [0u8; 4];
            let mut tha = [0u8; 6];
            let mut tpa = [0u8; 4];

            sha.copy_from_slice(&buf[22..28]);
            spa.copy_from_slice(&buf[28..32]);
            tha.copy_from_slice(&buf[32..38]);
            tpa.copy_from_slice(&buf[38..42]);

            RxDetail::Arp {
                oper,
                sha,
                spa,
                tha,
                tpa,
            }
        } else if ethertype == 0x0800 && len >= 34 {
            let proto = buf[23];

            let mut src = [0u8; 4];
            let mut dst = [0u8; 4];

            src.copy_from_slice(&buf[26..30]);
            dst.copy_from_slice(&buf[30..34]);

            RxDetail::Ipv4 { proto, src, dst }
        } else {
            RxDetail::None
        };

        RxState::Frame {
            len,
            dst_mac,
            src_mac,
            ethertype,
            detail,
        }
    }
}

impl App for HelloApp {
    fn title(&self) -> &'static str {
        "HELLO"
    }

    fn handle_event(&mut self, ev: &UiEvent) -> bool {
        let click_btn = Self::click_button_rect_local();
        let info_btn = Self::info_button_rect_local();
        let send_btn = Self::send_button_rect_local();
        let recv_btn = Self::recv_button_rect_local();

        match *ev {
            UiEvent::MouseMove { pos } => {
                let new_hover_click = click_btn.contains(pos);
                let new_hover_info = info_btn.contains(pos);
                let new_hover_send = send_btn.contains(pos);
                let new_hover_recv = recv_btn.contains(pos);

                if new_hover_click != self.hovered_click
                    || new_hover_info != self.hovered_info
                    || new_hover_send != self.hovered_send
                    || new_hover_recv != self.hovered_recv
                {
                    self.hovered_click = new_hover_click;
                    self.hovered_info = new_hover_info;
                    self.hovered_send = new_hover_send;
                    self.hovered_recv = new_hover_recv;
                    return true;
                }

                false
            }

            UiEvent::MouseDown {
                pos,
                button: MouseButton::Left,
            } => {
                let mut changed = false;

                if click_btn.contains(pos) && !self.pressed_click {
                    self.pressed_click = true;
                    changed = true;
                }
                if info_btn.contains(pos) && !self.pressed_info {
                    self.pressed_info = true;
                    changed = true;
                }
                if send_btn.contains(pos) && !self.pressed_send {
                    self.pressed_send = true;
                    changed = true;
                }
                if recv_btn.contains(pos) && !self.pressed_recv {
                    self.pressed_recv = true;
                    changed = true;
                }

                changed
            }

            UiEvent::MouseUp {
                pos,
                button: MouseButton::Left,
            } => {
                let hovered_click = click_btn.contains(pos);
                let hovered_info = info_btn.contains(pos);
                let hovered_send = send_btn.contains(pos);
                let hovered_recv = recv_btn.contains(pos);

                let was_pressed_click = self.pressed_click;
                let was_pressed_info = self.pressed_info;
                let was_pressed_send = self.pressed_send;
                let was_pressed_recv = self.pressed_recv;

                self.pressed_click = false;
                self.pressed_info = false;
                self.pressed_send = false;
                self.pressed_recv = false;

                self.hovered_click = hovered_click;
                self.hovered_info = hovered_info;
                self.hovered_send = hovered_send;
                self.hovered_recv = hovered_recv;

                let mut changed =
                    was_pressed_click || was_pressed_info || was_pressed_send || was_pressed_recv;

                if was_pressed_click && hovered_click {
                    self.count = self.count.wrapping_add(1);
                    changed = true;
                }

                if was_pressed_info && hovered_info {
                    self.refresh_nic_info();
                    changed = true;
                }

                if was_pressed_send && hovered_send {
                    self.send_arp_probe();
                    changed = true;
                }

                if was_pressed_recv && hovered_recv {
                    self.poll_rx();
                    changed = true;
                }

                changed
            }

            _ => false,
        }
    }

    fn render(&self, canvas: &mut Canvas, client_rect: Rect, _focused: bool) {
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 16,
            TEXT,
            None,
            "Embedded app",
        );

        let mut count_line = TextBuf::new();
        let _ = write!(&mut count_line, "Count={}", self.count);
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 32,
            TEXT,
            None,
            count_line.as_str(),
        );

        let click_btn = Self::click_button_rect_local().translate(client_rect.x, client_rect.y);
        draw_button(
            canvas,
            click_btn,
            "Click me",
            self.hovered_click,
            self.pressed_click,
        );

        let info_btn = Self::info_button_rect_local().translate(client_rect.x, client_rect.y);
        draw_button(
            canvas,
            info_btn,
            "Refresh NIC",
            self.hovered_info,
            self.pressed_info,
        );

        let send_btn = Self::send_button_rect_local().translate(client_rect.x, client_rect.y);
        draw_button(
            canvas,
            send_btn,
            "Send ARP",
            self.hovered_send,
            self.pressed_send,
        );

        let recv_btn = Self::recv_button_rect_local().translate(client_rect.x, client_rect.y);
        draw_button(
            canvas,
            recv_btn,
            "Poll RX",
            self.hovered_recv,
            self.pressed_recv,
        );

        let mut line1 = TextBuf::new();
        if self.nic_present {
            let _ = write!(
                &mut line1,
                "NIC: {}  MTU={}",
                if self.nic_link_up { "up" } else { "down" },
                self.nic_mtu
            );
        } else if let Some(e) = self.nic_info_err {
            let _ = write!(&mut line1, "NIC info errno={}", e);
        } else if self.nic_queried {
            let _ = write!(&mut line1, "NIC: not ready");
        } else {
            let _ = write!(&mut line1, "NIC: not queried");
        }
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 216,
            TEXT,
            None,
            line1.as_str(),
        );

        let mut line2 = TextBuf::new();
        if self.nic_present && self.nic_has_mac {
            let _ = write!(
                &mut line2,
                "MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                self.nic_mac[0],
                self.nic_mac[1],
                self.nic_mac[2],
                self.nic_mac[3],
                self.nic_mac[4],
                self.nic_mac[5]
            );
        } else if self.nic_present {
            let _ = write!(&mut line2, "MAC: (not advertised)");
        } else {
            let _ = write!(&mut line2, "MAC: -");
        }
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 232,
            TEXT,
            None,
            line2.as_str(),
        );

        let mut line3 = TextBuf::new();
        match self.tx_state {
            TxState::Idle => {
                let _ = write!(
                    &mut line3,
                    "TX: idle (ARP target {}.{}.{}.{})",
                    TEST_TARGET_IP[0], TEST_TARGET_IP[1], TEST_TARGET_IP[2], TEST_TARGET_IP[3]
                );
            }
            TxState::Ok(n) => {
                let _ = write!(&mut line3, "TX: sent {} bytes", n);
            }
            TxState::Err(e) => {
                let _ = write!(&mut line3, "TX: errno={}", e);
            }
        }
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 248,
            TEXT,
            None,
            line3.as_str(),
        );

        let mut line4 = TextBuf::new();
        let mut line5 = TextBuf::new();

        match self.rx_state {
            RxState::Idle => {
                let _ = write!(&mut line4, "RX: idle");
            }

            RxState::NoFrame => {
                let _ = write!(&mut line4, "RX: no frame available");
            }

            RxState::Err(e) => {
                let _ = write!(&mut line4, "RX: errno={}", e);
            }

            RxState::Frame {
                len,
                dst_mac,
                src_mac,
                ethertype,
                detail,
            } => {
                let _ = write!(
                    &mut line4,
                    "RX: len={} ethertype={:#06x} dst={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} src={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    len,
                    ethertype,
                    dst_mac[0],
                    dst_mac[1],
                    dst_mac[2],
                    dst_mac[3],
                    dst_mac[4],
                    dst_mac[5],
                    src_mac[0],
                    src_mac[1],
                    src_mac[2],
                    src_mac[3],
                    src_mac[4],
                    src_mac[5],
                );

                match detail {
                    RxDetail::None => {
                        let _ = write!(&mut line5, "RX detail: (unparsed)");
                    }

                    RxDetail::Arp {
                        oper,
                        sha,
                        spa,
                        tha,
                        tpa,
                    } => {
                        let _ = write!(
                            &mut line5,
                            "ARP op={} sha={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} spa={}.{}.{}.{} tha={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} tpa={}.{}.{}.{}",
                            oper,
                            sha[0],
                            sha[1],
                            sha[2],
                            sha[3],
                            sha[4],
                            sha[5],
                            spa[0],
                            spa[1],
                            spa[2],
                            spa[3],
                            tha[0],
                            tha[1],
                            tha[2],
                            tha[3],
                            tha[4],
                            tha[5],
                            tpa[0],
                            tpa[1],
                            tpa[2],
                            tpa[3],
                        );
                    }

                    RxDetail::Ipv4 { proto, src, dst } => {
                        let _ = write!(
                            &mut line5,
                            "IPv4 proto={} src={}.{}.{}.{} dst={}.{}.{}.{}",
                            proto, src[0], src[1], src[2], src[3], dst[0], dst[1], dst[2], dst[3],
                        );
                    }
                }
            }
        }

        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 264,
            TEXT,
            None,
            line4.as_str(),
        );
        draw_text(
            canvas,
            client_rect.x + 16,
            client_rect.y + 280,
            TEXT,
            None,
            line5.as_str(),
        );
    }
}

struct TextBuf {
    buf: [u8; 256],
    len: usize,
}

impl TextBuf {
    const fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl Write for TextBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let n = core::cmp::min(bytes.len(), self.buf.len().saturating_sub(self.len));
        self.buf[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
        Ok(())
    }
}
