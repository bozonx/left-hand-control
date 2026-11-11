//! Key codes the mapper engine works with: Linux `input-event-codes.h`
//! values on every OS. Linux uses `evdev::KeyCode` directly; other systems
//! use [`portable::KeyCode`], which has the same values and API, and their
//! backends translate native scan codes to and from it.

#[cfg(target_os = "linux")]
pub use evdev::KeyCode;
#[cfg(not(target_os = "linux"))]
pub use portable::KeyCode;

#[cfg(any(not(target_os = "linux"), test))]
pub mod portable {
    /// Linux key code (`KEY_*` / `BTN_*`) without the evdev dependency.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[repr(transparent)]
    pub struct KeyCode(pub u16);

    impl KeyCode {
        #[inline]
        pub const fn new(code: u16) -> Self {
            Self(code)
        }

        #[inline]
        pub const fn code(self) -> u16 {
            self.0
        }
    }

    // Generated from evdev 0.13.2 `scancodes.rs`.
    #[allow(dead_code)]
    impl KeyCode {
        pub const KEY_RESERVED: Self = Self(0);
        pub const KEY_ESC: Self = Self(1);
        pub const KEY_1: Self = Self(2);
        pub const KEY_2: Self = Self(3);
        pub const KEY_3: Self = Self(4);
        pub const KEY_4: Self = Self(5);
        pub const KEY_5: Self = Self(6);
        pub const KEY_6: Self = Self(7);
        pub const KEY_7: Self = Self(8);
        pub const KEY_8: Self = Self(9);
        pub const KEY_9: Self = Self(10);
        pub const KEY_0: Self = Self(11);
        pub const KEY_MINUS: Self = Self(12);
        pub const KEY_EQUAL: Self = Self(13);
        pub const KEY_BACKSPACE: Self = Self(14);
        pub const KEY_TAB: Self = Self(15);
        pub const KEY_Q: Self = Self(16);
        pub const KEY_W: Self = Self(17);
        pub const KEY_E: Self = Self(18);
        pub const KEY_R: Self = Self(19);
        pub const KEY_T: Self = Self(20);
        pub const KEY_Y: Self = Self(21);
        pub const KEY_U: Self = Self(22);
        pub const KEY_I: Self = Self(23);
        pub const KEY_O: Self = Self(24);
        pub const KEY_P: Self = Self(25);
        pub const KEY_LEFTBRACE: Self = Self(26);
        pub const KEY_RIGHTBRACE: Self = Self(27);
        pub const KEY_ENTER: Self = Self(28);
        pub const KEY_LEFTCTRL: Self = Self(29);
        pub const KEY_A: Self = Self(30);
        pub const KEY_S: Self = Self(31);
        pub const KEY_D: Self = Self(32);
        pub const KEY_F: Self = Self(33);
        pub const KEY_G: Self = Self(34);
        pub const KEY_H: Self = Self(35);
        pub const KEY_J: Self = Self(36);
        pub const KEY_K: Self = Self(37);
        pub const KEY_L: Self = Self(38);
        pub const KEY_SEMICOLON: Self = Self(39);
        pub const KEY_APOSTROPHE: Self = Self(40);
        pub const KEY_GRAVE: Self = Self(41);
        pub const KEY_LEFTSHIFT: Self = Self(42);
        pub const KEY_BACKSLASH: Self = Self(43);
        pub const KEY_Z: Self = Self(44);
        pub const KEY_X: Self = Self(45);
        pub const KEY_C: Self = Self(46);
        pub const KEY_V: Self = Self(47);
        pub const KEY_B: Self = Self(48);
        pub const KEY_N: Self = Self(49);
        pub const KEY_M: Self = Self(50);
        pub const KEY_COMMA: Self = Self(51);
        pub const KEY_DOT: Self = Self(52);
        pub const KEY_SLASH: Self = Self(53);
        pub const KEY_RIGHTSHIFT: Self = Self(54);
        pub const KEY_KPASTERISK: Self = Self(55);
        pub const KEY_LEFTALT: Self = Self(56);
        pub const KEY_SPACE: Self = Self(57);
        pub const KEY_CAPSLOCK: Self = Self(58);
        pub const KEY_F1: Self = Self(59);
        pub const KEY_F2: Self = Self(60);
        pub const KEY_F3: Self = Self(61);
        pub const KEY_F4: Self = Self(62);
        pub const KEY_F5: Self = Self(63);
        pub const KEY_F6: Self = Self(64);
        pub const KEY_F7: Self = Self(65);
        pub const KEY_F8: Self = Self(66);
        pub const KEY_F9: Self = Self(67);
        pub const KEY_F10: Self = Self(68);
        pub const KEY_NUMLOCK: Self = Self(69);
        pub const KEY_SCROLLLOCK: Self = Self(70);
        pub const KEY_KP7: Self = Self(71);
        pub const KEY_KP8: Self = Self(72);
        pub const KEY_KP9: Self = Self(73);
        pub const KEY_KPMINUS: Self = Self(74);
        pub const KEY_KP4: Self = Self(75);
        pub const KEY_KP5: Self = Self(76);
        pub const KEY_KP6: Self = Self(77);
        pub const KEY_KPPLUS: Self = Self(78);
        pub const KEY_KP1: Self = Self(79);
        pub const KEY_KP2: Self = Self(80);
        pub const KEY_KP3: Self = Self(81);
        pub const KEY_KP0: Self = Self(82);
        pub const KEY_KPDOT: Self = Self(83);
        pub const KEY_ZENKAKUHANKAKU: Self = Self(85);
        pub const KEY_102ND: Self = Self(86);
        pub const KEY_F11: Self = Self(87);
        pub const KEY_F12: Self = Self(88);
        pub const KEY_RO: Self = Self(89);
        pub const KEY_KATAKANA: Self = Self(90);
        pub const KEY_HIRAGANA: Self = Self(91);
        pub const KEY_HENKAN: Self = Self(92);
        pub const KEY_KATAKANAHIRAGANA: Self = Self(93);
        pub const KEY_MUHENKAN: Self = Self(94);
        pub const KEY_KPJPCOMMA: Self = Self(95);
        pub const KEY_KPENTER: Self = Self(96);
        pub const KEY_RIGHTCTRL: Self = Self(97);
        pub const KEY_KPSLASH: Self = Self(98);
        pub const KEY_SYSRQ: Self = Self(99);
        pub const KEY_RIGHTALT: Self = Self(100);
        pub const KEY_LINEFEED: Self = Self(101);
        pub const KEY_HOME: Self = Self(102);
        pub const KEY_UP: Self = Self(103);
        pub const KEY_PAGEUP: Self = Self(104);
        pub const KEY_LEFT: Self = Self(105);
        pub const KEY_RIGHT: Self = Self(106);
        pub const KEY_END: Self = Self(107);
        pub const KEY_DOWN: Self = Self(108);
        pub const KEY_PAGEDOWN: Self = Self(109);
        pub const KEY_INSERT: Self = Self(110);
        pub const KEY_DELETE: Self = Self(111);
        pub const KEY_MACRO: Self = Self(112);
        pub const KEY_MUTE: Self = Self(113);
        pub const KEY_VOLUMEDOWN: Self = Self(114);
        pub const KEY_VOLUMEUP: Self = Self(115);
        pub const KEY_POWER: Self = Self(116);
        pub const KEY_KPEQUAL: Self = Self(117);
        pub const KEY_KPPLUSMINUS: Self = Self(118);
        pub const KEY_PAUSE: Self = Self(119);
        pub const KEY_SCALE: Self = Self(120);
        pub const KEY_KPCOMMA: Self = Self(121);
        pub const KEY_HANGEUL: Self = Self(122);
        pub const KEY_HANJA: Self = Self(123);
        pub const KEY_YEN: Self = Self(124);
        pub const KEY_LEFTMETA: Self = Self(125);
        pub const KEY_RIGHTMETA: Self = Self(126);
        pub const KEY_COMPOSE: Self = Self(127);
        pub const KEY_STOP: Self = Self(128);
        pub const KEY_AGAIN: Self = Self(129);
        pub const KEY_PROPS: Self = Self(130);
        pub const KEY_UNDO: Self = Self(131);
        pub const KEY_FRONT: Self = Self(132);
        pub const KEY_COPY: Self = Self(133);
        pub const KEY_OPEN: Self = Self(134);
        pub const KEY_PASTE: Self = Self(135);
        pub const KEY_FIND: Self = Self(136);
        pub const KEY_CUT: Self = Self(137);
        pub const KEY_HELP: Self = Self(138);
        pub const KEY_MENU: Self = Self(139);
        pub const KEY_CALC: Self = Self(140);
        pub const KEY_SETUP: Self = Self(141);
        pub const KEY_SLEEP: Self = Self(142);
        pub const KEY_WAKEUP: Self = Self(143);
        pub const KEY_FILE: Self = Self(144);
        pub const KEY_SENDFILE: Self = Self(145);
        pub const KEY_DELETEFILE: Self = Self(146);
        pub const KEY_XFER: Self = Self(147);
        pub const KEY_PROG1: Self = Self(148);
        pub const KEY_PROG2: Self = Self(149);
        pub const KEY_WWW: Self = Self(150);
        pub const KEY_MSDOS: Self = Self(151);
        pub const KEY_COFFEE: Self = Self(152);
        pub const KEY_DIRECTION: Self = Self(153);
        pub const KEY_ROTATE_DISPLAY: Self = Self(153);
        pub const KEY_CYCLEWINDOWS: Self = Self(154);
        pub const KEY_MAIL: Self = Self(155);
        pub const KEY_BOOKMARKS: Self = Self(156);
        pub const KEY_COMPUTER: Self = Self(157);
        pub const KEY_BACK: Self = Self(158);
        pub const KEY_FORWARD: Self = Self(159);
        pub const KEY_CLOSECD: Self = Self(160);
        pub const KEY_EJECTCD: Self = Self(161);
        pub const KEY_EJECTCLOSECD: Self = Self(162);
        pub const KEY_NEXTSONG: Self = Self(163);
        pub const KEY_PLAYPAUSE: Self = Self(164);
        pub const KEY_PREVIOUSSONG: Self = Self(165);
        pub const KEY_STOPCD: Self = Self(166);
        pub const KEY_RECORD: Self = Self(167);
        pub const KEY_REWIND: Self = Self(168);
        pub const KEY_PHONE: Self = Self(169);
        pub const KEY_ISO: Self = Self(170);
        pub const KEY_CONFIG: Self = Self(171);
        pub const KEY_HOMEPAGE: Self = Self(172);
        pub const KEY_REFRESH: Self = Self(173);
        pub const KEY_EXIT: Self = Self(174);
        pub const KEY_MOVE: Self = Self(175);
        pub const KEY_EDIT: Self = Self(176);
        pub const KEY_SCROLLUP: Self = Self(177);
        pub const KEY_SCROLLDOWN: Self = Self(178);
        pub const KEY_KPLEFTPAREN: Self = Self(179);
        pub const KEY_KPRIGHTPAREN: Self = Self(180);
        pub const KEY_NEW: Self = Self(181);
        pub const KEY_REDO: Self = Self(182);
        pub const KEY_F13: Self = Self(183);
        pub const KEY_F14: Self = Self(184);
        pub const KEY_F15: Self = Self(185);
        pub const KEY_F16: Self = Self(186);
        pub const KEY_F17: Self = Self(187);
        pub const KEY_F18: Self = Self(188);
        pub const KEY_F19: Self = Self(189);
        pub const KEY_F20: Self = Self(190);
        pub const KEY_F21: Self = Self(191);
        pub const KEY_F22: Self = Self(192);
        pub const KEY_F23: Self = Self(193);
        pub const KEY_F24: Self = Self(194);
        pub const KEY_PLAYCD: Self = Self(200);
        pub const KEY_PAUSECD: Self = Self(201);
        pub const KEY_PROG3: Self = Self(202);
        pub const KEY_PROG4: Self = Self(203);
        pub const KEY_DASHBOARD: Self = Self(204);
        pub const KEY_SUSPEND: Self = Self(205);
        pub const KEY_CLOSE: Self = Self(206);
        pub const KEY_PLAY: Self = Self(207);
        pub const KEY_FASTFORWARD: Self = Self(208);
        pub const KEY_BASSBOOST: Self = Self(209);
        pub const KEY_PRINT: Self = Self(210);
        pub const KEY_HP: Self = Self(211);
        pub const KEY_CAMERA: Self = Self(212);
        pub const KEY_SOUND: Self = Self(213);
        pub const KEY_QUESTION: Self = Self(214);
        pub const KEY_EMAIL: Self = Self(215);
        pub const KEY_CHAT: Self = Self(216);
        pub const KEY_SEARCH: Self = Self(217);
        pub const KEY_CONNECT: Self = Self(218);
        pub const KEY_FINANCE: Self = Self(219);
        pub const KEY_SPORT: Self = Self(220);
        pub const KEY_SHOP: Self = Self(221);
        pub const KEY_ALTERASE: Self = Self(222);
        pub const KEY_CANCEL: Self = Self(223);
        pub const KEY_BRIGHTNESSDOWN: Self = Self(224);
        pub const KEY_BRIGHTNESSUP: Self = Self(225);
        pub const KEY_MEDIA: Self = Self(226);
        pub const KEY_SWITCHVIDEOMODE: Self = Self(227);
        pub const KEY_KBDILLUMTOGGLE: Self = Self(228);
        pub const KEY_KBDILLUMDOWN: Self = Self(229);
        pub const KEY_KBDILLUMUP: Self = Self(230);
        pub const KEY_SEND: Self = Self(231);
        pub const KEY_REPLY: Self = Self(232);
        pub const KEY_FORWARDMAIL: Self = Self(233);
        pub const KEY_SAVE: Self = Self(234);
        pub const KEY_DOCUMENTS: Self = Self(235);
        pub const KEY_BATTERY: Self = Self(236);
        pub const KEY_BLUETOOTH: Self = Self(237);
        pub const KEY_WLAN: Self = Self(238);
        pub const KEY_UWB: Self = Self(239);
        pub const KEY_UNKNOWN: Self = Self(240);
        pub const KEY_VIDEO_NEXT: Self = Self(241);
        pub const KEY_VIDEO_PREV: Self = Self(242);
        pub const KEY_BRIGHTNESS_CYCLE: Self = Self(243);
        pub const KEY_BRIGHTNESS_AUTO: Self = Self(244);
        pub const KEY_DISPLAY_OFF: Self = Self(245);
        pub const KEY_WWAN: Self = Self(246);
        pub const KEY_RFKILL: Self = Self(247);
        pub const KEY_MICMUTE: Self = Self(248);
        pub const BTN_0: Self = Self(0x100);
        pub const BTN_1: Self = Self(0x101);
        pub const BTN_2: Self = Self(0x102);
        pub const BTN_3: Self = Self(0x103);
        pub const BTN_4: Self = Self(0x104);
        pub const BTN_5: Self = Self(0x105);
        pub const BTN_6: Self = Self(0x106);
        pub const BTN_7: Self = Self(0x107);
        pub const BTN_8: Self = Self(0x108);
        pub const BTN_9: Self = Self(0x109);
        pub const BTN_LEFT: Self = Self(0x110);
        pub const BTN_RIGHT: Self = Self(0x111);
        pub const BTN_MIDDLE: Self = Self(0x112);
        pub const BTN_SIDE: Self = Self(0x113);
        pub const BTN_EXTRA: Self = Self(0x114);
        pub const BTN_FORWARD: Self = Self(0x115);
        pub const BTN_BACK: Self = Self(0x116);
        pub const BTN_TASK: Self = Self(0x117);
        pub const BTN_TRIGGER: Self = Self(0x120);
        pub const BTN_THUMB: Self = Self(0x121);
        pub const BTN_THUMB2: Self = Self(0x122);
        pub const BTN_TOP: Self = Self(0x123);
        pub const BTN_TOP2: Self = Self(0x124);
        pub const BTN_PINKIE: Self = Self(0x125);
        pub const BTN_BASE: Self = Self(0x126);
        pub const BTN_BASE2: Self = Self(0x127);
        pub const BTN_BASE3: Self = Self(0x128);
        pub const BTN_BASE4: Self = Self(0x129);
        pub const BTN_BASE5: Self = Self(0x12a);
        pub const BTN_BASE6: Self = Self(0x12b);
        pub const BTN_DEAD: Self = Self(0x12f);
        pub const BTN_SOUTH: Self = Self(0x130);
        pub const BTN_EAST: Self = Self(0x131);
        pub const BTN_C: Self = Self(0x132);
        pub const BTN_NORTH: Self = Self(0x133);
        pub const BTN_WEST: Self = Self(0x134);
        pub const BTN_Z: Self = Self(0x135);
        pub const BTN_TL: Self = Self(0x136);
        pub const BTN_TR: Self = Self(0x137);
        pub const BTN_TL2: Self = Self(0x138);
        pub const BTN_TR2: Self = Self(0x139);
        pub const BTN_SELECT: Self = Self(0x13a);
        pub const BTN_START: Self = Self(0x13b);
        pub const BTN_MODE: Self = Self(0x13c);
        pub const BTN_THUMBL: Self = Self(0x13d);
        pub const BTN_THUMBR: Self = Self(0x13e);
        pub const BTN_TOOL_PEN: Self = Self(0x140);
        pub const BTN_TOOL_RUBBER: Self = Self(0x141);
        pub const BTN_TOOL_BRUSH: Self = Self(0x142);
        pub const BTN_TOOL_PENCIL: Self = Self(0x143);
        pub const BTN_TOOL_AIRBRUSH: Self = Self(0x144);
        pub const BTN_TOOL_FINGER: Self = Self(0x145);
        pub const BTN_TOOL_MOUSE: Self = Self(0x146);
        pub const BTN_TOOL_LENS: Self = Self(0x147);
        pub const BTN_TOOL_QUINTTAP: Self = Self(0x148);
        pub const BTN_TOUCH: Self = Self(0x14a);
        pub const BTN_STYLUS: Self = Self(0x14b);
        pub const BTN_STYLUS2: Self = Self(0x14c);
        pub const BTN_TOOL_DOUBLETAP: Self = Self(0x14d);
        pub const BTN_TOOL_TRIPLETAP: Self = Self(0x14e);
        pub const BTN_TOOL_QUADTAP: Self = Self(0x14f);
        pub const BTN_GEAR_DOWN: Self = Self(0x150);
        pub const BTN_GEAR_UP: Self = Self(0x151);
        pub const KEY_OK: Self = Self(0x160);
        pub const KEY_SELECT: Self = Self(0x161);
        pub const KEY_GOTO: Self = Self(0x162);
        pub const KEY_CLEAR: Self = Self(0x163);
        pub const KEY_POWER2: Self = Self(0x164);
        pub const KEY_OPTION: Self = Self(0x165);
        pub const KEY_INFO: Self = Self(0x166);
        pub const KEY_TIME: Self = Self(0x167);
        pub const KEY_VENDOR: Self = Self(0x168);
        pub const KEY_ARCHIVE: Self = Self(0x169);
        pub const KEY_PROGRAM: Self = Self(0x16a);
        pub const KEY_CHANNEL: Self = Self(0x16b);
        pub const KEY_FAVORITES: Self = Self(0x16c);
        pub const KEY_EPG: Self = Self(0x16d);
        pub const KEY_PVR: Self = Self(0x16e);
        pub const KEY_MHP: Self = Self(0x16f);
        pub const KEY_LANGUAGE: Self = Self(0x170);
        pub const KEY_TITLE: Self = Self(0x171);
        pub const KEY_SUBTITLE: Self = Self(0x172);
        pub const KEY_ANGLE: Self = Self(0x173);
        pub const KEY_ZOOM: Self = Self(0x174);
        pub const KEY_FULL_SCREEN: Self = Self(0x174);
        pub const KEY_MODE: Self = Self(0x175);
        pub const KEY_KEYBOARD: Self = Self(0x176);
        pub const KEY_SCREEN: Self = Self(0x177);
        pub const KEY_PC: Self = Self(0x178);
        pub const KEY_TV: Self = Self(0x179);
        pub const KEY_TV2: Self = Self(0x17a);
        pub const KEY_VCR: Self = Self(0x17b);
        pub const KEY_VCR2: Self = Self(0x17c);
        pub const KEY_SAT: Self = Self(0x17d);
        pub const KEY_SAT2: Self = Self(0x17e);
        pub const KEY_CD: Self = Self(0x17f);
        pub const KEY_TAPE: Self = Self(0x180);
        pub const KEY_RADIO: Self = Self(0x181);
        pub const KEY_TUNER: Self = Self(0x182);
        pub const KEY_PLAYER: Self = Self(0x183);
        pub const KEY_TEXT: Self = Self(0x184);
        pub const KEY_DVD: Self = Self(0x185);
        pub const KEY_AUX: Self = Self(0x186);
        pub const KEY_MP3: Self = Self(0x187);
        pub const KEY_AUDIO: Self = Self(0x188);
        pub const KEY_VIDEO: Self = Self(0x189);
        pub const KEY_DIRECTORY: Self = Self(0x18a);
        pub const KEY_LIST: Self = Self(0x18b);
        pub const KEY_MEMO: Self = Self(0x18c);
        pub const KEY_CALENDAR: Self = Self(0x18d);
        pub const KEY_RED: Self = Self(0x18e);
        pub const KEY_GREEN: Self = Self(0x18f);
        pub const KEY_YELLOW: Self = Self(0x190);
        pub const KEY_BLUE: Self = Self(0x191);
        pub const KEY_CHANNELUP: Self = Self(0x192);
        pub const KEY_CHANNELDOWN: Self = Self(0x193);
        pub const KEY_FIRST: Self = Self(0x194);
        pub const KEY_LAST: Self = Self(0x195);
        pub const KEY_AB: Self = Self(0x196);
        pub const KEY_NEXT: Self = Self(0x197);
        pub const KEY_RESTART: Self = Self(0x198);
        pub const KEY_SLOW: Self = Self(0x199);
        pub const KEY_SHUFFLE: Self = Self(0x19a);
        pub const KEY_BREAK: Self = Self(0x19b);
        pub const KEY_PREVIOUS: Self = Self(0x19c);
        pub const KEY_DIGITS: Self = Self(0x19d);
        pub const KEY_TEEN: Self = Self(0x19e);
        pub const KEY_TWEN: Self = Self(0x19f);
        pub const KEY_VIDEOPHONE: Self = Self(0x1a0);
        pub const KEY_GAMES: Self = Self(0x1a1);
        pub const KEY_ZOOMIN: Self = Self(0x1a2);
        pub const KEY_ZOOMOUT: Self = Self(0x1a3);
        pub const KEY_ZOOMRESET: Self = Self(0x1a4);
        pub const KEY_WORDPROCESSOR: Self = Self(0x1a5);
        pub const KEY_EDITOR: Self = Self(0x1a6);
        pub const KEY_SPREADSHEET: Self = Self(0x1a7);
        pub const KEY_GRAPHICSEDITOR: Self = Self(0x1a8);
        pub const KEY_PRESENTATION: Self = Self(0x1a9);
        pub const KEY_DATABASE: Self = Self(0x1aa);
        pub const KEY_NEWS: Self = Self(0x1ab);
        pub const KEY_VOICEMAIL: Self = Self(0x1ac);
        pub const KEY_ADDRESSBOOK: Self = Self(0x1ad);
        pub const KEY_MESSENGER: Self = Self(0x1ae);
        pub const KEY_DISPLAYTOGGLE: Self = Self(0x1af);
        pub const KEY_SPELLCHECK: Self = Self(0x1b0);
        pub const KEY_LOGOFF: Self = Self(0x1b1);
        pub const KEY_DOLLAR: Self = Self(0x1b2);
        pub const KEY_EURO: Self = Self(0x1b3);
        pub const KEY_FRAMEBACK: Self = Self(0x1b4);
        pub const KEY_FRAMEFORWARD: Self = Self(0x1b5);
        pub const KEY_CONTEXT_MENU: Self = Self(0x1b6);
        pub const KEY_MEDIA_REPEAT: Self = Self(0x1b7);
        pub const KEY_10CHANNELSUP: Self = Self(0x1b8);
        pub const KEY_10CHANNELSDOWN: Self = Self(0x1b9);
        pub const KEY_IMAGES: Self = Self(0x1ba);
        pub const KEY_PICKUP_PHONE: Self = Self(0x1bd);
        pub const KEY_HANGUP_PHONE: Self = Self(0x1be);
        pub const KEY_DEL_EOL: Self = Self(0x1c0);
        pub const KEY_DEL_EOS: Self = Self(0x1c1);
        pub const KEY_INS_LINE: Self = Self(0x1c2);
        pub const KEY_DEL_LINE: Self = Self(0x1c3);
        pub const KEY_FN: Self = Self(0x1d0);
        pub const KEY_FN_ESC: Self = Self(0x1d1);
        pub const KEY_FN_F1: Self = Self(0x1d2);
        pub const KEY_FN_F2: Self = Self(0x1d3);
        pub const KEY_FN_F3: Self = Self(0x1d4);
        pub const KEY_FN_F4: Self = Self(0x1d5);
        pub const KEY_FN_F5: Self = Self(0x1d6);
        pub const KEY_FN_F6: Self = Self(0x1d7);
        pub const KEY_FN_F7: Self = Self(0x1d8);
        pub const KEY_FN_F8: Self = Self(0x1d9);
        pub const KEY_FN_F9: Self = Self(0x1da);
        pub const KEY_FN_F10: Self = Self(0x1db);
        pub const KEY_FN_F11: Self = Self(0x1dc);
        pub const KEY_FN_F12: Self = Self(0x1dd);
        pub const KEY_FN_1: Self = Self(0x1de);
        pub const KEY_FN_2: Self = Self(0x1df);
        pub const KEY_FN_D: Self = Self(0x1e0);
        pub const KEY_FN_E: Self = Self(0x1e1);
        pub const KEY_FN_F: Self = Self(0x1e2);
        pub const KEY_FN_S: Self = Self(0x1e3);
        pub const KEY_FN_B: Self = Self(0x1e4);
        pub const KEY_BRL_DOT1: Self = Self(0x1f1);
        pub const KEY_BRL_DOT2: Self = Self(0x1f2);
        pub const KEY_BRL_DOT3: Self = Self(0x1f3);
        pub const KEY_BRL_DOT4: Self = Self(0x1f4);
        pub const KEY_BRL_DOT5: Self = Self(0x1f5);
        pub const KEY_BRL_DOT6: Self = Self(0x1f6);
        pub const KEY_BRL_DOT7: Self = Self(0x1f7);
        pub const KEY_BRL_DOT8: Self = Self(0x1f8);
        pub const KEY_BRL_DOT9: Self = Self(0x1f9);
        pub const KEY_BRL_DOT10: Self = Self(0x1fa);
        pub const KEY_NUMERIC_0: Self = Self(0x200);
        pub const KEY_NUMERIC_1: Self = Self(0x201);
        pub const KEY_NUMERIC_2: Self = Self(0x202);
        pub const KEY_NUMERIC_3: Self = Self(0x203);
        pub const KEY_NUMERIC_4: Self = Self(0x204);
        pub const KEY_NUMERIC_5: Self = Self(0x205);
        pub const KEY_NUMERIC_6: Self = Self(0x206);
        pub const KEY_NUMERIC_7: Self = Self(0x207);
        pub const KEY_NUMERIC_8: Self = Self(0x208);
        pub const KEY_NUMERIC_9: Self = Self(0x209);
        pub const KEY_NUMERIC_STAR: Self = Self(0x20a);
        pub const KEY_NUMERIC_POUND: Self = Self(0x20b);
        pub const KEY_NUMERIC_A: Self = Self(0x20c);
        pub const KEY_NUMERIC_B: Self = Self(0x20d);
        pub const KEY_NUMERIC_C: Self = Self(0x20e);
        pub const KEY_NUMERIC_D: Self = Self(0x20f);
        pub const KEY_CAMERA_FOCUS: Self = Self(0x210);
        pub const KEY_WPS_BUTTON: Self = Self(0x211);
        pub const KEY_TOUCHPAD_TOGGLE: Self = Self(0x212);
        pub const KEY_TOUCHPAD_ON: Self = Self(0x213);
        pub const KEY_TOUCHPAD_OFF: Self = Self(0x214);
        pub const KEY_CAMERA_ZOOMIN: Self = Self(0x215);
        pub const KEY_CAMERA_ZOOMOUT: Self = Self(0x216);
        pub const KEY_CAMERA_UP: Self = Self(0x217);
        pub const KEY_CAMERA_DOWN: Self = Self(0x218);
        pub const KEY_CAMERA_LEFT: Self = Self(0x219);
        pub const KEY_CAMERA_RIGHT: Self = Self(0x21a);
        pub const KEY_ATTENDANT_ON: Self = Self(0x21b);
        pub const KEY_ATTENDANT_OFF: Self = Self(0x21c);
        pub const KEY_ATTENDANT_TOGGLE: Self = Self(0x21d);
        pub const KEY_LIGHTS_TOGGLE: Self = Self(0x21e);
        pub const BTN_DPAD_UP: Self = Self(0x220);
        pub const BTN_DPAD_DOWN: Self = Self(0x221);
        pub const BTN_DPAD_LEFT: Self = Self(0x222);
        pub const BTN_DPAD_RIGHT: Self = Self(0x223);
        pub const KEY_ALS_TOGGLE: Self = Self(0x230);
        pub const KEY_BUTTONCONFIG: Self = Self(0x240);
        pub const KEY_TASKMANAGER: Self = Self(0x241);
        pub const KEY_JOURNAL: Self = Self(0x242);
        pub const KEY_CONTROLPANEL: Self = Self(0x243);
        pub const KEY_APPSELECT: Self = Self(0x244);
        pub const KEY_SCREENSAVER: Self = Self(0x245);
        pub const KEY_VOICECOMMAND: Self = Self(0x246);
        pub const KEY_ASSISTANT: Self = Self(0x247);
        pub const KEY_KBD_LAYOUT_NEXT: Self = Self(0x248);
        pub const KEY_BRIGHTNESS_MIN: Self = Self(0x250);
        pub const KEY_BRIGHTNESS_MAX: Self = Self(0x251);
        pub const KEY_KBDINPUTASSIST_PREV: Self = Self(0x260);
        pub const KEY_KBDINPUTASSIST_NEXT: Self = Self(0x261);
        pub const KEY_KBDINPUTASSIST_PREVGROUP: Self = Self(0x262);
        pub const KEY_KBDINPUTASSIST_NEXTGROUP: Self = Self(0x263);
        pub const KEY_KBDINPUTASSIST_ACCEPT: Self = Self(0x264);
        pub const KEY_KBDINPUTASSIST_CANCEL: Self = Self(0x265);
        pub const KEY_RIGHT_UP: Self = Self(0x266);
        pub const KEY_RIGHT_DOWN: Self = Self(0x267);
        pub const KEY_LEFT_UP: Self = Self(0x268);
        pub const KEY_LEFT_DOWN: Self = Self(0x269);
        pub const KEY_ROOT_MENU: Self = Self(0x26a);
        pub const KEY_MEDIA_TOP_MENU: Self = Self(0x26b);
        pub const KEY_NUMERIC_11: Self = Self(0x26c);
        pub const KEY_NUMERIC_12: Self = Self(0x26d);
        pub const KEY_AUDIO_DESC: Self = Self(0x26e);
        pub const KEY_3D_MODE: Self = Self(0x26f);
        pub const KEY_NEXT_FAVORITE: Self = Self(0x270);
        pub const KEY_STOP_RECORD: Self = Self(0x271);
        pub const KEY_PAUSE_RECORD: Self = Self(0x272);
        pub const KEY_VOD: Self = Self(0x273);
        pub const KEY_UNMUTE: Self = Self(0x274);
        pub const KEY_FASTREVERSE: Self = Self(0x275);
        pub const KEY_SLOWREVERSE: Self = Self(0x276);
        pub const KEY_DATA: Self = Self(0x277);
        pub const KEY_ONSCREEN_KEYBOARD: Self = Self(0x278);
        pub const KEY_PRIVACY_SCREEN_TOGGLE: Self = Self(0x279);
        pub const KEY_SELECTIVE_SCREENSHOT: Self = Self(0x27a);
        pub const BTN_TRIGGER_HAPPY1: Self = Self(0x2c0);
        pub const BTN_TRIGGER_HAPPY2: Self = Self(0x2c1);
        pub const BTN_TRIGGER_HAPPY3: Self = Self(0x2c2);
        pub const BTN_TRIGGER_HAPPY4: Self = Self(0x2c3);
        pub const BTN_TRIGGER_HAPPY5: Self = Self(0x2c4);
        pub const BTN_TRIGGER_HAPPY6: Self = Self(0x2c5);
        pub const BTN_TRIGGER_HAPPY7: Self = Self(0x2c6);
        pub const BTN_TRIGGER_HAPPY8: Self = Self(0x2c7);
        pub const BTN_TRIGGER_HAPPY9: Self = Self(0x2c8);
        pub const BTN_TRIGGER_HAPPY10: Self = Self(0x2c9);
        pub const BTN_TRIGGER_HAPPY11: Self = Self(0x2ca);
        pub const BTN_TRIGGER_HAPPY12: Self = Self(0x2cb);
        pub const BTN_TRIGGER_HAPPY13: Self = Self(0x2cc);
        pub const BTN_TRIGGER_HAPPY14: Self = Self(0x2cd);
        pub const BTN_TRIGGER_HAPPY15: Self = Self(0x2ce);
        pub const BTN_TRIGGER_HAPPY16: Self = Self(0x2cf);
        pub const BTN_TRIGGER_HAPPY17: Self = Self(0x2d0);
        pub const BTN_TRIGGER_HAPPY18: Self = Self(0x2d1);
        pub const BTN_TRIGGER_HAPPY19: Self = Self(0x2d2);
        pub const BTN_TRIGGER_HAPPY20: Self = Self(0x2d3);
        pub const BTN_TRIGGER_HAPPY21: Self = Self(0x2d4);
        pub const BTN_TRIGGER_HAPPY22: Self = Self(0x2d5);
        pub const BTN_TRIGGER_HAPPY23: Self = Self(0x2d6);
        pub const BTN_TRIGGER_HAPPY24: Self = Self(0x2d7);
        pub const BTN_TRIGGER_HAPPY25: Self = Self(0x2d8);
        pub const BTN_TRIGGER_HAPPY26: Self = Self(0x2d9);
        pub const BTN_TRIGGER_HAPPY27: Self = Self(0x2da);
        pub const BTN_TRIGGER_HAPPY28: Self = Self(0x2db);
        pub const BTN_TRIGGER_HAPPY29: Self = Self(0x2dc);
        pub const BTN_TRIGGER_HAPPY30: Self = Self(0x2dd);
        pub const BTN_TRIGGER_HAPPY31: Self = Self(0x2de);
        pub const BTN_TRIGGER_HAPPY32: Self = Self(0x2df);
        pub const BTN_TRIGGER_HAPPY33: Self = Self(0x2e0);
        pub const BTN_TRIGGER_HAPPY34: Self = Self(0x2e1);
        pub const BTN_TRIGGER_HAPPY35: Self = Self(0x2e2);
        pub const BTN_TRIGGER_HAPPY36: Self = Self(0x2e3);
        pub const BTN_TRIGGER_HAPPY37: Self = Self(0x2e4);
        pub const BTN_TRIGGER_HAPPY38: Self = Self(0x2e5);
        pub const BTN_TRIGGER_HAPPY39: Self = Self(0x2e6);
        pub const BTN_TRIGGER_HAPPY40: Self = Self(0x2e7);
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(super) const ALL: &[(KeyCode, u16)] = &[
        (KeyCode::KEY_RESERVED, evdev::KeyCode::KEY_RESERVED.code()),
        (KeyCode::KEY_ESC, evdev::KeyCode::KEY_ESC.code()),
        (KeyCode::KEY_1, evdev::KeyCode::KEY_1.code()),
        (KeyCode::KEY_2, evdev::KeyCode::KEY_2.code()),
        (KeyCode::KEY_3, evdev::KeyCode::KEY_3.code()),
        (KeyCode::KEY_4, evdev::KeyCode::KEY_4.code()),
        (KeyCode::KEY_5, evdev::KeyCode::KEY_5.code()),
        (KeyCode::KEY_6, evdev::KeyCode::KEY_6.code()),
        (KeyCode::KEY_7, evdev::KeyCode::KEY_7.code()),
        (KeyCode::KEY_8, evdev::KeyCode::KEY_8.code()),
        (KeyCode::KEY_9, evdev::KeyCode::KEY_9.code()),
        (KeyCode::KEY_0, evdev::KeyCode::KEY_0.code()),
        (KeyCode::KEY_MINUS, evdev::KeyCode::KEY_MINUS.code()),
        (KeyCode::KEY_EQUAL, evdev::KeyCode::KEY_EQUAL.code()),
        (KeyCode::KEY_BACKSPACE, evdev::KeyCode::KEY_BACKSPACE.code()),
        (KeyCode::KEY_TAB, evdev::KeyCode::KEY_TAB.code()),
        (KeyCode::KEY_Q, evdev::KeyCode::KEY_Q.code()),
        (KeyCode::KEY_W, evdev::KeyCode::KEY_W.code()),
        (KeyCode::KEY_E, evdev::KeyCode::KEY_E.code()),
        (KeyCode::KEY_R, evdev::KeyCode::KEY_R.code()),
        (KeyCode::KEY_T, evdev::KeyCode::KEY_T.code()),
        (KeyCode::KEY_Y, evdev::KeyCode::KEY_Y.code()),
        (KeyCode::KEY_U, evdev::KeyCode::KEY_U.code()),
        (KeyCode::KEY_I, evdev::KeyCode::KEY_I.code()),
        (KeyCode::KEY_O, evdev::KeyCode::KEY_O.code()),
        (KeyCode::KEY_P, evdev::KeyCode::KEY_P.code()),
        (KeyCode::KEY_LEFTBRACE, evdev::KeyCode::KEY_LEFTBRACE.code()),
        (
            KeyCode::KEY_RIGHTBRACE,
            evdev::KeyCode::KEY_RIGHTBRACE.code(),
        ),
        (KeyCode::KEY_ENTER, evdev::KeyCode::KEY_ENTER.code()),
        (KeyCode::KEY_LEFTCTRL, evdev::KeyCode::KEY_LEFTCTRL.code()),
        (KeyCode::KEY_A, evdev::KeyCode::KEY_A.code()),
        (KeyCode::KEY_S, evdev::KeyCode::KEY_S.code()),
        (KeyCode::KEY_D, evdev::KeyCode::KEY_D.code()),
        (KeyCode::KEY_F, evdev::KeyCode::KEY_F.code()),
        (KeyCode::KEY_G, evdev::KeyCode::KEY_G.code()),
        (KeyCode::KEY_H, evdev::KeyCode::KEY_H.code()),
        (KeyCode::KEY_J, evdev::KeyCode::KEY_J.code()),
        (KeyCode::KEY_K, evdev::KeyCode::KEY_K.code()),
        (KeyCode::KEY_L, evdev::KeyCode::KEY_L.code()),
        (KeyCode::KEY_SEMICOLON, evdev::KeyCode::KEY_SEMICOLON.code()),
        (
            KeyCode::KEY_APOSTROPHE,
            evdev::KeyCode::KEY_APOSTROPHE.code(),
        ),
        (KeyCode::KEY_GRAVE, evdev::KeyCode::KEY_GRAVE.code()),
        (KeyCode::KEY_LEFTSHIFT, evdev::KeyCode::KEY_LEFTSHIFT.code()),
        (KeyCode::KEY_BACKSLASH, evdev::KeyCode::KEY_BACKSLASH.code()),
        (KeyCode::KEY_Z, evdev::KeyCode::KEY_Z.code()),
        (KeyCode::KEY_X, evdev::KeyCode::KEY_X.code()),
        (KeyCode::KEY_C, evdev::KeyCode::KEY_C.code()),
        (KeyCode::KEY_V, evdev::KeyCode::KEY_V.code()),
        (KeyCode::KEY_B, evdev::KeyCode::KEY_B.code()),
        (KeyCode::KEY_N, evdev::KeyCode::KEY_N.code()),
        (KeyCode::KEY_M, evdev::KeyCode::KEY_M.code()),
        (KeyCode::KEY_COMMA, evdev::KeyCode::KEY_COMMA.code()),
        (KeyCode::KEY_DOT, evdev::KeyCode::KEY_DOT.code()),
        (KeyCode::KEY_SLASH, evdev::KeyCode::KEY_SLASH.code()),
        (
            KeyCode::KEY_RIGHTSHIFT,
            evdev::KeyCode::KEY_RIGHTSHIFT.code(),
        ),
        (
            KeyCode::KEY_KPASTERISK,
            evdev::KeyCode::KEY_KPASTERISK.code(),
        ),
        (KeyCode::KEY_LEFTALT, evdev::KeyCode::KEY_LEFTALT.code()),
        (KeyCode::KEY_SPACE, evdev::KeyCode::KEY_SPACE.code()),
        (KeyCode::KEY_CAPSLOCK, evdev::KeyCode::KEY_CAPSLOCK.code()),
        (KeyCode::KEY_F1, evdev::KeyCode::KEY_F1.code()),
        (KeyCode::KEY_F2, evdev::KeyCode::KEY_F2.code()),
        (KeyCode::KEY_F3, evdev::KeyCode::KEY_F3.code()),
        (KeyCode::KEY_F4, evdev::KeyCode::KEY_F4.code()),
        (KeyCode::KEY_F5, evdev::KeyCode::KEY_F5.code()),
        (KeyCode::KEY_F6, evdev::KeyCode::KEY_F6.code()),
        (KeyCode::KEY_F7, evdev::KeyCode::KEY_F7.code()),
        (KeyCode::KEY_F8, evdev::KeyCode::KEY_F8.code()),
        (KeyCode::KEY_F9, evdev::KeyCode::KEY_F9.code()),
        (KeyCode::KEY_F10, evdev::KeyCode::KEY_F10.code()),
        (KeyCode::KEY_NUMLOCK, evdev::KeyCode::KEY_NUMLOCK.code()),
        (
            KeyCode::KEY_SCROLLLOCK,
            evdev::KeyCode::KEY_SCROLLLOCK.code(),
        ),
        (KeyCode::KEY_KP7, evdev::KeyCode::KEY_KP7.code()),
        (KeyCode::KEY_KP8, evdev::KeyCode::KEY_KP8.code()),
        (KeyCode::KEY_KP9, evdev::KeyCode::KEY_KP9.code()),
        (KeyCode::KEY_KPMINUS, evdev::KeyCode::KEY_KPMINUS.code()),
        (KeyCode::KEY_KP4, evdev::KeyCode::KEY_KP4.code()),
        (KeyCode::KEY_KP5, evdev::KeyCode::KEY_KP5.code()),
        (KeyCode::KEY_KP6, evdev::KeyCode::KEY_KP6.code()),
        (KeyCode::KEY_KPPLUS, evdev::KeyCode::KEY_KPPLUS.code()),
        (KeyCode::KEY_KP1, evdev::KeyCode::KEY_KP1.code()),
        (KeyCode::KEY_KP2, evdev::KeyCode::KEY_KP2.code()),
        (KeyCode::KEY_KP3, evdev::KeyCode::KEY_KP3.code()),
        (KeyCode::KEY_KP0, evdev::KeyCode::KEY_KP0.code()),
        (KeyCode::KEY_KPDOT, evdev::KeyCode::KEY_KPDOT.code()),
        (
            KeyCode::KEY_ZENKAKUHANKAKU,
            evdev::KeyCode::KEY_ZENKAKUHANKAKU.code(),
        ),
        (KeyCode::KEY_102ND, evdev::KeyCode::KEY_102ND.code()),
        (KeyCode::KEY_F11, evdev::KeyCode::KEY_F11.code()),
        (KeyCode::KEY_F12, evdev::KeyCode::KEY_F12.code()),
        (KeyCode::KEY_RO, evdev::KeyCode::KEY_RO.code()),
        (KeyCode::KEY_KATAKANA, evdev::KeyCode::KEY_KATAKANA.code()),
        (KeyCode::KEY_HIRAGANA, evdev::KeyCode::KEY_HIRAGANA.code()),
        (KeyCode::KEY_HENKAN, evdev::KeyCode::KEY_HENKAN.code()),
        (
            KeyCode::KEY_KATAKANAHIRAGANA,
            evdev::KeyCode::KEY_KATAKANAHIRAGANA.code(),
        ),
        (KeyCode::KEY_MUHENKAN, evdev::KeyCode::KEY_MUHENKAN.code()),
        (KeyCode::KEY_KPJPCOMMA, evdev::KeyCode::KEY_KPJPCOMMA.code()),
        (KeyCode::KEY_KPENTER, evdev::KeyCode::KEY_KPENTER.code()),
        (KeyCode::KEY_RIGHTCTRL, evdev::KeyCode::KEY_RIGHTCTRL.code()),
        (KeyCode::KEY_KPSLASH, evdev::KeyCode::KEY_KPSLASH.code()),
        (KeyCode::KEY_SYSRQ, evdev::KeyCode::KEY_SYSRQ.code()),
        (KeyCode::KEY_RIGHTALT, evdev::KeyCode::KEY_RIGHTALT.code()),
        (KeyCode::KEY_LINEFEED, evdev::KeyCode::KEY_LINEFEED.code()),
        (KeyCode::KEY_HOME, evdev::KeyCode::KEY_HOME.code()),
        (KeyCode::KEY_UP, evdev::KeyCode::KEY_UP.code()),
        (KeyCode::KEY_PAGEUP, evdev::KeyCode::KEY_PAGEUP.code()),
        (KeyCode::KEY_LEFT, evdev::KeyCode::KEY_LEFT.code()),
        (KeyCode::KEY_RIGHT, evdev::KeyCode::KEY_RIGHT.code()),
        (KeyCode::KEY_END, evdev::KeyCode::KEY_END.code()),
        (KeyCode::KEY_DOWN, evdev::KeyCode::KEY_DOWN.code()),
        (KeyCode::KEY_PAGEDOWN, evdev::KeyCode::KEY_PAGEDOWN.code()),
        (KeyCode::KEY_INSERT, evdev::KeyCode::KEY_INSERT.code()),
        (KeyCode::KEY_DELETE, evdev::KeyCode::KEY_DELETE.code()),
        (KeyCode::KEY_MACRO, evdev::KeyCode::KEY_MACRO.code()),
        (KeyCode::KEY_MUTE, evdev::KeyCode::KEY_MUTE.code()),
        (
            KeyCode::KEY_VOLUMEDOWN,
            evdev::KeyCode::KEY_VOLUMEDOWN.code(),
        ),
        (KeyCode::KEY_VOLUMEUP, evdev::KeyCode::KEY_VOLUMEUP.code()),
        (KeyCode::KEY_POWER, evdev::KeyCode::KEY_POWER.code()),
        (KeyCode::KEY_KPEQUAL, evdev::KeyCode::KEY_KPEQUAL.code()),
        (
            KeyCode::KEY_KPPLUSMINUS,
            evdev::KeyCode::KEY_KPPLUSMINUS.code(),
        ),
        (KeyCode::KEY_PAUSE, evdev::KeyCode::KEY_PAUSE.code()),
        (KeyCode::KEY_SCALE, evdev::KeyCode::KEY_SCALE.code()),
        (KeyCode::KEY_KPCOMMA, evdev::KeyCode::KEY_KPCOMMA.code()),
        (KeyCode::KEY_HANGEUL, evdev::KeyCode::KEY_HANGEUL.code()),
        (KeyCode::KEY_HANJA, evdev::KeyCode::KEY_HANJA.code()),
        (KeyCode::KEY_YEN, evdev::KeyCode::KEY_YEN.code()),
        (KeyCode::KEY_LEFTMETA, evdev::KeyCode::KEY_LEFTMETA.code()),
        (KeyCode::KEY_RIGHTMETA, evdev::KeyCode::KEY_RIGHTMETA.code()),
        (KeyCode::KEY_COMPOSE, evdev::KeyCode::KEY_COMPOSE.code()),
        (KeyCode::KEY_STOP, evdev::KeyCode::KEY_STOP.code()),
        (KeyCode::KEY_AGAIN, evdev::KeyCode::KEY_AGAIN.code()),
        (KeyCode::KEY_PROPS, evdev::KeyCode::KEY_PROPS.code()),
        (KeyCode::KEY_UNDO, evdev::KeyCode::KEY_UNDO.code()),
        (KeyCode::KEY_FRONT, evdev::KeyCode::KEY_FRONT.code()),
        (KeyCode::KEY_COPY, evdev::KeyCode::KEY_COPY.code()),
        (KeyCode::KEY_OPEN, evdev::KeyCode::KEY_OPEN.code()),
        (KeyCode::KEY_PASTE, evdev::KeyCode::KEY_PASTE.code()),
        (KeyCode::KEY_FIND, evdev::KeyCode::KEY_FIND.code()),
        (KeyCode::KEY_CUT, evdev::KeyCode::KEY_CUT.code()),
        (KeyCode::KEY_HELP, evdev::KeyCode::KEY_HELP.code()),
        (KeyCode::KEY_MENU, evdev::KeyCode::KEY_MENU.code()),
        (KeyCode::KEY_CALC, evdev::KeyCode::KEY_CALC.code()),
        (KeyCode::KEY_SETUP, evdev::KeyCode::KEY_SETUP.code()),
        (KeyCode::KEY_SLEEP, evdev::KeyCode::KEY_SLEEP.code()),
        (KeyCode::KEY_WAKEUP, evdev::KeyCode::KEY_WAKEUP.code()),
        (KeyCode::KEY_FILE, evdev::KeyCode::KEY_FILE.code()),
        (KeyCode::KEY_SENDFILE, evdev::KeyCode::KEY_SENDFILE.code()),
        (
            KeyCode::KEY_DELETEFILE,
            evdev::KeyCode::KEY_DELETEFILE.code(),
        ),
        (KeyCode::KEY_XFER, evdev::KeyCode::KEY_XFER.code()),
        (KeyCode::KEY_PROG1, evdev::KeyCode::KEY_PROG1.code()),
        (KeyCode::KEY_PROG2, evdev::KeyCode::KEY_PROG2.code()),
        (KeyCode::KEY_WWW, evdev::KeyCode::KEY_WWW.code()),
        (KeyCode::KEY_MSDOS, evdev::KeyCode::KEY_MSDOS.code()),
        (KeyCode::KEY_COFFEE, evdev::KeyCode::KEY_COFFEE.code()),
        (KeyCode::KEY_DIRECTION, evdev::KeyCode::KEY_DIRECTION.code()),
        (
            KeyCode::KEY_ROTATE_DISPLAY,
            evdev::KeyCode::KEY_ROTATE_DISPLAY.code(),
        ),
        (
            KeyCode::KEY_CYCLEWINDOWS,
            evdev::KeyCode::KEY_CYCLEWINDOWS.code(),
        ),
        (KeyCode::KEY_MAIL, evdev::KeyCode::KEY_MAIL.code()),
        (KeyCode::KEY_BOOKMARKS, evdev::KeyCode::KEY_BOOKMARKS.code()),
        (KeyCode::KEY_COMPUTER, evdev::KeyCode::KEY_COMPUTER.code()),
        (KeyCode::KEY_BACK, evdev::KeyCode::KEY_BACK.code()),
        (KeyCode::KEY_FORWARD, evdev::KeyCode::KEY_FORWARD.code()),
        (KeyCode::KEY_CLOSECD, evdev::KeyCode::KEY_CLOSECD.code()),
        (KeyCode::KEY_EJECTCD, evdev::KeyCode::KEY_EJECTCD.code()),
        (
            KeyCode::KEY_EJECTCLOSECD,
            evdev::KeyCode::KEY_EJECTCLOSECD.code(),
        ),
        (KeyCode::KEY_NEXTSONG, evdev::KeyCode::KEY_NEXTSONG.code()),
        (KeyCode::KEY_PLAYPAUSE, evdev::KeyCode::KEY_PLAYPAUSE.code()),
        (
            KeyCode::KEY_PREVIOUSSONG,
            evdev::KeyCode::KEY_PREVIOUSSONG.code(),
        ),
        (KeyCode::KEY_STOPCD, evdev::KeyCode::KEY_STOPCD.code()),
        (KeyCode::KEY_RECORD, evdev::KeyCode::KEY_RECORD.code()),
        (KeyCode::KEY_REWIND, evdev::KeyCode::KEY_REWIND.code()),
        (KeyCode::KEY_PHONE, evdev::KeyCode::KEY_PHONE.code()),
        (KeyCode::KEY_ISO, evdev::KeyCode::KEY_ISO.code()),
        (KeyCode::KEY_CONFIG, evdev::KeyCode::KEY_CONFIG.code()),
        (KeyCode::KEY_HOMEPAGE, evdev::KeyCode::KEY_HOMEPAGE.code()),
        (KeyCode::KEY_REFRESH, evdev::KeyCode::KEY_REFRESH.code()),
        (KeyCode::KEY_EXIT, evdev::KeyCode::KEY_EXIT.code()),
        (KeyCode::KEY_MOVE, evdev::KeyCode::KEY_MOVE.code()),
        (KeyCode::KEY_EDIT, evdev::KeyCode::KEY_EDIT.code()),
        (KeyCode::KEY_SCROLLUP, evdev::KeyCode::KEY_SCROLLUP.code()),
        (
            KeyCode::KEY_SCROLLDOWN,
            evdev::KeyCode::KEY_SCROLLDOWN.code(),
        ),
        (
            KeyCode::KEY_KPLEFTPAREN,
            evdev::KeyCode::KEY_KPLEFTPAREN.code(),
        ),
        (
            KeyCode::KEY_KPRIGHTPAREN,
            evdev::KeyCode::KEY_KPRIGHTPAREN.code(),
        ),
        (KeyCode::KEY_NEW, evdev::KeyCode::KEY_NEW.code()),
        (KeyCode::KEY_REDO, evdev::KeyCode::KEY_REDO.code()),
        (KeyCode::KEY_F13, evdev::KeyCode::KEY_F13.code()),
        (KeyCode::KEY_F14, evdev::KeyCode::KEY_F14.code()),
        (KeyCode::KEY_F15, evdev::KeyCode::KEY_F15.code()),
        (KeyCode::KEY_F16, evdev::KeyCode::KEY_F16.code()),
        (KeyCode::KEY_F17, evdev::KeyCode::KEY_F17.code()),
        (KeyCode::KEY_F18, evdev::KeyCode::KEY_F18.code()),
        (KeyCode::KEY_F19, evdev::KeyCode::KEY_F19.code()),
        (KeyCode::KEY_F20, evdev::KeyCode::KEY_F20.code()),
        (KeyCode::KEY_F21, evdev::KeyCode::KEY_F21.code()),
        (KeyCode::KEY_F22, evdev::KeyCode::KEY_F22.code()),
        (KeyCode::KEY_F23, evdev::KeyCode::KEY_F23.code()),
        (KeyCode::KEY_F24, evdev::KeyCode::KEY_F24.code()),
        (KeyCode::KEY_PLAYCD, evdev::KeyCode::KEY_PLAYCD.code()),
        (KeyCode::KEY_PAUSECD, evdev::KeyCode::KEY_PAUSECD.code()),
        (KeyCode::KEY_PROG3, evdev::KeyCode::KEY_PROG3.code()),
        (KeyCode::KEY_PROG4, evdev::KeyCode::KEY_PROG4.code()),
        (KeyCode::KEY_DASHBOARD, evdev::KeyCode::KEY_DASHBOARD.code()),
        (KeyCode::KEY_SUSPEND, evdev::KeyCode::KEY_SUSPEND.code()),
        (KeyCode::KEY_CLOSE, evdev::KeyCode::KEY_CLOSE.code()),
        (KeyCode::KEY_PLAY, evdev::KeyCode::KEY_PLAY.code()),
        (
            KeyCode::KEY_FASTFORWARD,
            evdev::KeyCode::KEY_FASTFORWARD.code(),
        ),
        (KeyCode::KEY_BASSBOOST, evdev::KeyCode::KEY_BASSBOOST.code()),
        (KeyCode::KEY_PRINT, evdev::KeyCode::KEY_PRINT.code()),
        (KeyCode::KEY_HP, evdev::KeyCode::KEY_HP.code()),
        (KeyCode::KEY_CAMERA, evdev::KeyCode::KEY_CAMERA.code()),
        (KeyCode::KEY_SOUND, evdev::KeyCode::KEY_SOUND.code()),
        (KeyCode::KEY_QUESTION, evdev::KeyCode::KEY_QUESTION.code()),
        (KeyCode::KEY_EMAIL, evdev::KeyCode::KEY_EMAIL.code()),
        (KeyCode::KEY_CHAT, evdev::KeyCode::KEY_CHAT.code()),
        (KeyCode::KEY_SEARCH, evdev::KeyCode::KEY_SEARCH.code()),
        (KeyCode::KEY_CONNECT, evdev::KeyCode::KEY_CONNECT.code()),
        (KeyCode::KEY_FINANCE, evdev::KeyCode::KEY_FINANCE.code()),
        (KeyCode::KEY_SPORT, evdev::KeyCode::KEY_SPORT.code()),
        (KeyCode::KEY_SHOP, evdev::KeyCode::KEY_SHOP.code()),
        (KeyCode::KEY_ALTERASE, evdev::KeyCode::KEY_ALTERASE.code()),
        (KeyCode::KEY_CANCEL, evdev::KeyCode::KEY_CANCEL.code()),
        (
            KeyCode::KEY_BRIGHTNESSDOWN,
            evdev::KeyCode::KEY_BRIGHTNESSDOWN.code(),
        ),
        (
            KeyCode::KEY_BRIGHTNESSUP,
            evdev::KeyCode::KEY_BRIGHTNESSUP.code(),
        ),
        (KeyCode::KEY_MEDIA, evdev::KeyCode::KEY_MEDIA.code()),
        (
            KeyCode::KEY_SWITCHVIDEOMODE,
            evdev::KeyCode::KEY_SWITCHVIDEOMODE.code(),
        ),
        (
            KeyCode::KEY_KBDILLUMTOGGLE,
            evdev::KeyCode::KEY_KBDILLUMTOGGLE.code(),
        ),
        (
            KeyCode::KEY_KBDILLUMDOWN,
            evdev::KeyCode::KEY_KBDILLUMDOWN.code(),
        ),
        (
            KeyCode::KEY_KBDILLUMUP,
            evdev::KeyCode::KEY_KBDILLUMUP.code(),
        ),
        (KeyCode::KEY_SEND, evdev::KeyCode::KEY_SEND.code()),
        (KeyCode::KEY_REPLY, evdev::KeyCode::KEY_REPLY.code()),
        (
            KeyCode::KEY_FORWARDMAIL,
            evdev::KeyCode::KEY_FORWARDMAIL.code(),
        ),
        (KeyCode::KEY_SAVE, evdev::KeyCode::KEY_SAVE.code()),
        (KeyCode::KEY_DOCUMENTS, evdev::KeyCode::KEY_DOCUMENTS.code()),
        (KeyCode::KEY_BATTERY, evdev::KeyCode::KEY_BATTERY.code()),
        (KeyCode::KEY_BLUETOOTH, evdev::KeyCode::KEY_BLUETOOTH.code()),
        (KeyCode::KEY_WLAN, evdev::KeyCode::KEY_WLAN.code()),
        (KeyCode::KEY_UWB, evdev::KeyCode::KEY_UWB.code()),
        (KeyCode::KEY_UNKNOWN, evdev::KeyCode::KEY_UNKNOWN.code()),
        (
            KeyCode::KEY_VIDEO_NEXT,
            evdev::KeyCode::KEY_VIDEO_NEXT.code(),
        ),
        (
            KeyCode::KEY_VIDEO_PREV,
            evdev::KeyCode::KEY_VIDEO_PREV.code(),
        ),
        (
            KeyCode::KEY_BRIGHTNESS_CYCLE,
            evdev::KeyCode::KEY_BRIGHTNESS_CYCLE.code(),
        ),
        (
            KeyCode::KEY_BRIGHTNESS_AUTO,
            evdev::KeyCode::KEY_BRIGHTNESS_AUTO.code(),
        ),
        (
            KeyCode::KEY_DISPLAY_OFF,
            evdev::KeyCode::KEY_DISPLAY_OFF.code(),
        ),
        (KeyCode::KEY_WWAN, evdev::KeyCode::KEY_WWAN.code()),
        (KeyCode::KEY_RFKILL, evdev::KeyCode::KEY_RFKILL.code()),
        (KeyCode::KEY_MICMUTE, evdev::KeyCode::KEY_MICMUTE.code()),
        (KeyCode::BTN_0, evdev::KeyCode::BTN_0.code()),
        (KeyCode::BTN_1, evdev::KeyCode::BTN_1.code()),
        (KeyCode::BTN_2, evdev::KeyCode::BTN_2.code()),
        (KeyCode::BTN_3, evdev::KeyCode::BTN_3.code()),
        (KeyCode::BTN_4, evdev::KeyCode::BTN_4.code()),
        (KeyCode::BTN_5, evdev::KeyCode::BTN_5.code()),
        (KeyCode::BTN_6, evdev::KeyCode::BTN_6.code()),
        (KeyCode::BTN_7, evdev::KeyCode::BTN_7.code()),
        (KeyCode::BTN_8, evdev::KeyCode::BTN_8.code()),
        (KeyCode::BTN_9, evdev::KeyCode::BTN_9.code()),
        (KeyCode::BTN_LEFT, evdev::KeyCode::BTN_LEFT.code()),
        (KeyCode::BTN_RIGHT, evdev::KeyCode::BTN_RIGHT.code()),
        (KeyCode::BTN_MIDDLE, evdev::KeyCode::BTN_MIDDLE.code()),
        (KeyCode::BTN_SIDE, evdev::KeyCode::BTN_SIDE.code()),
        (KeyCode::BTN_EXTRA, evdev::KeyCode::BTN_EXTRA.code()),
        (KeyCode::BTN_FORWARD, evdev::KeyCode::BTN_FORWARD.code()),
        (KeyCode::BTN_BACK, evdev::KeyCode::BTN_BACK.code()),
        (KeyCode::BTN_TASK, evdev::KeyCode::BTN_TASK.code()),
        (KeyCode::BTN_TRIGGER, evdev::KeyCode::BTN_TRIGGER.code()),
        (KeyCode::BTN_THUMB, evdev::KeyCode::BTN_THUMB.code()),
        (KeyCode::BTN_THUMB2, evdev::KeyCode::BTN_THUMB2.code()),
        (KeyCode::BTN_TOP, evdev::KeyCode::BTN_TOP.code()),
        (KeyCode::BTN_TOP2, evdev::KeyCode::BTN_TOP2.code()),
        (KeyCode::BTN_PINKIE, evdev::KeyCode::BTN_PINKIE.code()),
        (KeyCode::BTN_BASE, evdev::KeyCode::BTN_BASE.code()),
        (KeyCode::BTN_BASE2, evdev::KeyCode::BTN_BASE2.code()),
        (KeyCode::BTN_BASE3, evdev::KeyCode::BTN_BASE3.code()),
        (KeyCode::BTN_BASE4, evdev::KeyCode::BTN_BASE4.code()),
        (KeyCode::BTN_BASE5, evdev::KeyCode::BTN_BASE5.code()),
        (KeyCode::BTN_BASE6, evdev::KeyCode::BTN_BASE6.code()),
        (KeyCode::BTN_DEAD, evdev::KeyCode::BTN_DEAD.code()),
        (KeyCode::BTN_SOUTH, evdev::KeyCode::BTN_SOUTH.code()),
        (KeyCode::BTN_EAST, evdev::KeyCode::BTN_EAST.code()),
        (KeyCode::BTN_C, evdev::KeyCode::BTN_C.code()),
        (KeyCode::BTN_NORTH, evdev::KeyCode::BTN_NORTH.code()),
        (KeyCode::BTN_WEST, evdev::KeyCode::BTN_WEST.code()),
        (KeyCode::BTN_Z, evdev::KeyCode::BTN_Z.code()),
        (KeyCode::BTN_TL, evdev::KeyCode::BTN_TL.code()),
        (KeyCode::BTN_TR, evdev::KeyCode::BTN_TR.code()),
        (KeyCode::BTN_TL2, evdev::KeyCode::BTN_TL2.code()),
        (KeyCode::BTN_TR2, evdev::KeyCode::BTN_TR2.code()),
        (KeyCode::BTN_SELECT, evdev::KeyCode::BTN_SELECT.code()),
        (KeyCode::BTN_START, evdev::KeyCode::BTN_START.code()),
        (KeyCode::BTN_MODE, evdev::KeyCode::BTN_MODE.code()),
        (KeyCode::BTN_THUMBL, evdev::KeyCode::BTN_THUMBL.code()),
        (KeyCode::BTN_THUMBR, evdev::KeyCode::BTN_THUMBR.code()),
        (KeyCode::BTN_TOOL_PEN, evdev::KeyCode::BTN_TOOL_PEN.code()),
        (
            KeyCode::BTN_TOOL_RUBBER,
            evdev::KeyCode::BTN_TOOL_RUBBER.code(),
        ),
        (
            KeyCode::BTN_TOOL_BRUSH,
            evdev::KeyCode::BTN_TOOL_BRUSH.code(),
        ),
        (
            KeyCode::BTN_TOOL_PENCIL,
            evdev::KeyCode::BTN_TOOL_PENCIL.code(),
        ),
        (
            KeyCode::BTN_TOOL_AIRBRUSH,
            evdev::KeyCode::BTN_TOOL_AIRBRUSH.code(),
        ),
        (
            KeyCode::BTN_TOOL_FINGER,
            evdev::KeyCode::BTN_TOOL_FINGER.code(),
        ),
        (
            KeyCode::BTN_TOOL_MOUSE,
            evdev::KeyCode::BTN_TOOL_MOUSE.code(),
        ),
        (KeyCode::BTN_TOOL_LENS, evdev::KeyCode::BTN_TOOL_LENS.code()),
        (
            KeyCode::BTN_TOOL_QUINTTAP,
            evdev::KeyCode::BTN_TOOL_QUINTTAP.code(),
        ),
        (KeyCode::BTN_TOUCH, evdev::KeyCode::BTN_TOUCH.code()),
        (KeyCode::BTN_STYLUS, evdev::KeyCode::BTN_STYLUS.code()),
        (KeyCode::BTN_STYLUS2, evdev::KeyCode::BTN_STYLUS2.code()),
        (
            KeyCode::BTN_TOOL_DOUBLETAP,
            evdev::KeyCode::BTN_TOOL_DOUBLETAP.code(),
        ),
        (
            KeyCode::BTN_TOOL_TRIPLETAP,
            evdev::KeyCode::BTN_TOOL_TRIPLETAP.code(),
        ),
        (
            KeyCode::BTN_TOOL_QUADTAP,
            evdev::KeyCode::BTN_TOOL_QUADTAP.code(),
        ),
        (KeyCode::BTN_GEAR_DOWN, evdev::KeyCode::BTN_GEAR_DOWN.code()),
        (KeyCode::BTN_GEAR_UP, evdev::KeyCode::BTN_GEAR_UP.code()),
        (KeyCode::KEY_OK, evdev::KeyCode::KEY_OK.code()),
        (KeyCode::KEY_SELECT, evdev::KeyCode::KEY_SELECT.code()),
        (KeyCode::KEY_GOTO, evdev::KeyCode::KEY_GOTO.code()),
        (KeyCode::KEY_CLEAR, evdev::KeyCode::KEY_CLEAR.code()),
        (KeyCode::KEY_POWER2, evdev::KeyCode::KEY_POWER2.code()),
        (KeyCode::KEY_OPTION, evdev::KeyCode::KEY_OPTION.code()),
        (KeyCode::KEY_INFO, evdev::KeyCode::KEY_INFO.code()),
        (KeyCode::KEY_TIME, evdev::KeyCode::KEY_TIME.code()),
        (KeyCode::KEY_VENDOR, evdev::KeyCode::KEY_VENDOR.code()),
        (KeyCode::KEY_ARCHIVE, evdev::KeyCode::KEY_ARCHIVE.code()),
        (KeyCode::KEY_PROGRAM, evdev::KeyCode::KEY_PROGRAM.code()),
        (KeyCode::KEY_CHANNEL, evdev::KeyCode::KEY_CHANNEL.code()),
        (KeyCode::KEY_FAVORITES, evdev::KeyCode::KEY_FAVORITES.code()),
        (KeyCode::KEY_EPG, evdev::KeyCode::KEY_EPG.code()),
        (KeyCode::KEY_PVR, evdev::KeyCode::KEY_PVR.code()),
        (KeyCode::KEY_MHP, evdev::KeyCode::KEY_MHP.code()),
        (KeyCode::KEY_LANGUAGE, evdev::KeyCode::KEY_LANGUAGE.code()),
        (KeyCode::KEY_TITLE, evdev::KeyCode::KEY_TITLE.code()),
        (KeyCode::KEY_SUBTITLE, evdev::KeyCode::KEY_SUBTITLE.code()),
        (KeyCode::KEY_ANGLE, evdev::KeyCode::KEY_ANGLE.code()),
        (KeyCode::KEY_ZOOM, evdev::KeyCode::KEY_ZOOM.code()),
        (
            KeyCode::KEY_FULL_SCREEN,
            evdev::KeyCode::KEY_FULL_SCREEN.code(),
        ),
        (KeyCode::KEY_MODE, evdev::KeyCode::KEY_MODE.code()),
        (KeyCode::KEY_KEYBOARD, evdev::KeyCode::KEY_KEYBOARD.code()),
        (KeyCode::KEY_SCREEN, evdev::KeyCode::KEY_SCREEN.code()),
        (KeyCode::KEY_PC, evdev::KeyCode::KEY_PC.code()),
        (KeyCode::KEY_TV, evdev::KeyCode::KEY_TV.code()),
        (KeyCode::KEY_TV2, evdev::KeyCode::KEY_TV2.code()),
        (KeyCode::KEY_VCR, evdev::KeyCode::KEY_VCR.code()),
        (KeyCode::KEY_VCR2, evdev::KeyCode::KEY_VCR2.code()),
        (KeyCode::KEY_SAT, evdev::KeyCode::KEY_SAT.code()),
        (KeyCode::KEY_SAT2, evdev::KeyCode::KEY_SAT2.code()),
        (KeyCode::KEY_CD, evdev::KeyCode::KEY_CD.code()),
        (KeyCode::KEY_TAPE, evdev::KeyCode::KEY_TAPE.code()),
        (KeyCode::KEY_RADIO, evdev::KeyCode::KEY_RADIO.code()),
        (KeyCode::KEY_TUNER, evdev::KeyCode::KEY_TUNER.code()),
        (KeyCode::KEY_PLAYER, evdev::KeyCode::KEY_PLAYER.code()),
        (KeyCode::KEY_TEXT, evdev::KeyCode::KEY_TEXT.code()),
        (KeyCode::KEY_DVD, evdev::KeyCode::KEY_DVD.code()),
        (KeyCode::KEY_AUX, evdev::KeyCode::KEY_AUX.code()),
        (KeyCode::KEY_MP3, evdev::KeyCode::KEY_MP3.code()),
        (KeyCode::KEY_AUDIO, evdev::KeyCode::KEY_AUDIO.code()),
        (KeyCode::KEY_VIDEO, evdev::KeyCode::KEY_VIDEO.code()),
        (KeyCode::KEY_DIRECTORY, evdev::KeyCode::KEY_DIRECTORY.code()),
        (KeyCode::KEY_LIST, evdev::KeyCode::KEY_LIST.code()),
        (KeyCode::KEY_MEMO, evdev::KeyCode::KEY_MEMO.code()),
        (KeyCode::KEY_CALENDAR, evdev::KeyCode::KEY_CALENDAR.code()),
        (KeyCode::KEY_RED, evdev::KeyCode::KEY_RED.code()),
        (KeyCode::KEY_GREEN, evdev::KeyCode::KEY_GREEN.code()),
        (KeyCode::KEY_YELLOW, evdev::KeyCode::KEY_YELLOW.code()),
        (KeyCode::KEY_BLUE, evdev::KeyCode::KEY_BLUE.code()),
        (KeyCode::KEY_CHANNELUP, evdev::KeyCode::KEY_CHANNELUP.code()),
        (
            KeyCode::KEY_CHANNELDOWN,
            evdev::KeyCode::KEY_CHANNELDOWN.code(),
        ),
        (KeyCode::KEY_FIRST, evdev::KeyCode::KEY_FIRST.code()),
        (KeyCode::KEY_LAST, evdev::KeyCode::KEY_LAST.code()),
        (KeyCode::KEY_AB, evdev::KeyCode::KEY_AB.code()),
        (KeyCode::KEY_NEXT, evdev::KeyCode::KEY_NEXT.code()),
        (KeyCode::KEY_RESTART, evdev::KeyCode::KEY_RESTART.code()),
        (KeyCode::KEY_SLOW, evdev::KeyCode::KEY_SLOW.code()),
        (KeyCode::KEY_SHUFFLE, evdev::KeyCode::KEY_SHUFFLE.code()),
        (KeyCode::KEY_BREAK, evdev::KeyCode::KEY_BREAK.code()),
        (KeyCode::KEY_PREVIOUS, evdev::KeyCode::KEY_PREVIOUS.code()),
        (KeyCode::KEY_DIGITS, evdev::KeyCode::KEY_DIGITS.code()),
        (KeyCode::KEY_TEEN, evdev::KeyCode::KEY_TEEN.code()),
        (KeyCode::KEY_TWEN, evdev::KeyCode::KEY_TWEN.code()),
        (
            KeyCode::KEY_VIDEOPHONE,
            evdev::KeyCode::KEY_VIDEOPHONE.code(),
        ),
        (KeyCode::KEY_GAMES, evdev::KeyCode::KEY_GAMES.code()),
        (KeyCode::KEY_ZOOMIN, evdev::KeyCode::KEY_ZOOMIN.code()),
        (KeyCode::KEY_ZOOMOUT, evdev::KeyCode::KEY_ZOOMOUT.code()),
        (KeyCode::KEY_ZOOMRESET, evdev::KeyCode::KEY_ZOOMRESET.code()),
        (
            KeyCode::KEY_WORDPROCESSOR,
            evdev::KeyCode::KEY_WORDPROCESSOR.code(),
        ),
        (KeyCode::KEY_EDITOR, evdev::KeyCode::KEY_EDITOR.code()),
        (
            KeyCode::KEY_SPREADSHEET,
            evdev::KeyCode::KEY_SPREADSHEET.code(),
        ),
        (
            KeyCode::KEY_GRAPHICSEDITOR,
            evdev::KeyCode::KEY_GRAPHICSEDITOR.code(),
        ),
        (
            KeyCode::KEY_PRESENTATION,
            evdev::KeyCode::KEY_PRESENTATION.code(),
        ),
        (KeyCode::KEY_DATABASE, evdev::KeyCode::KEY_DATABASE.code()),
        (KeyCode::KEY_NEWS, evdev::KeyCode::KEY_NEWS.code()),
        (KeyCode::KEY_VOICEMAIL, evdev::KeyCode::KEY_VOICEMAIL.code()),
        (
            KeyCode::KEY_ADDRESSBOOK,
            evdev::KeyCode::KEY_ADDRESSBOOK.code(),
        ),
        (KeyCode::KEY_MESSENGER, evdev::KeyCode::KEY_MESSENGER.code()),
        (
            KeyCode::KEY_DISPLAYTOGGLE,
            evdev::KeyCode::KEY_DISPLAYTOGGLE.code(),
        ),
        (
            KeyCode::KEY_SPELLCHECK,
            evdev::KeyCode::KEY_SPELLCHECK.code(),
        ),
        (KeyCode::KEY_LOGOFF, evdev::KeyCode::KEY_LOGOFF.code()),
        (KeyCode::KEY_DOLLAR, evdev::KeyCode::KEY_DOLLAR.code()),
        (KeyCode::KEY_EURO, evdev::KeyCode::KEY_EURO.code()),
        (KeyCode::KEY_FRAMEBACK, evdev::KeyCode::KEY_FRAMEBACK.code()),
        (
            KeyCode::KEY_FRAMEFORWARD,
            evdev::KeyCode::KEY_FRAMEFORWARD.code(),
        ),
        (
            KeyCode::KEY_CONTEXT_MENU,
            evdev::KeyCode::KEY_CONTEXT_MENU.code(),
        ),
        (
            KeyCode::KEY_MEDIA_REPEAT,
            evdev::KeyCode::KEY_MEDIA_REPEAT.code(),
        ),
        (
            KeyCode::KEY_10CHANNELSUP,
            evdev::KeyCode::KEY_10CHANNELSUP.code(),
        ),
        (
            KeyCode::KEY_10CHANNELSDOWN,
            evdev::KeyCode::KEY_10CHANNELSDOWN.code(),
        ),
        (KeyCode::KEY_IMAGES, evdev::KeyCode::KEY_IMAGES.code()),
        (
            KeyCode::KEY_PICKUP_PHONE,
            evdev::KeyCode::KEY_PICKUP_PHONE.code(),
        ),
        (
            KeyCode::KEY_HANGUP_PHONE,
            evdev::KeyCode::KEY_HANGUP_PHONE.code(),
        ),
        (KeyCode::KEY_DEL_EOL, evdev::KeyCode::KEY_DEL_EOL.code()),
        (KeyCode::KEY_DEL_EOS, evdev::KeyCode::KEY_DEL_EOS.code()),
        (KeyCode::KEY_INS_LINE, evdev::KeyCode::KEY_INS_LINE.code()),
        (KeyCode::KEY_DEL_LINE, evdev::KeyCode::KEY_DEL_LINE.code()),
        (KeyCode::KEY_FN, evdev::KeyCode::KEY_FN.code()),
        (KeyCode::KEY_FN_ESC, evdev::KeyCode::KEY_FN_ESC.code()),
        (KeyCode::KEY_FN_F1, evdev::KeyCode::KEY_FN_F1.code()),
        (KeyCode::KEY_FN_F2, evdev::KeyCode::KEY_FN_F2.code()),
        (KeyCode::KEY_FN_F3, evdev::KeyCode::KEY_FN_F3.code()),
        (KeyCode::KEY_FN_F4, evdev::KeyCode::KEY_FN_F4.code()),
        (KeyCode::KEY_FN_F5, evdev::KeyCode::KEY_FN_F5.code()),
        (KeyCode::KEY_FN_F6, evdev::KeyCode::KEY_FN_F6.code()),
        (KeyCode::KEY_FN_F7, evdev::KeyCode::KEY_FN_F7.code()),
        (KeyCode::KEY_FN_F8, evdev::KeyCode::KEY_FN_F8.code()),
        (KeyCode::KEY_FN_F9, evdev::KeyCode::KEY_FN_F9.code()),
        (KeyCode::KEY_FN_F10, evdev::KeyCode::KEY_FN_F10.code()),
        (KeyCode::KEY_FN_F11, evdev::KeyCode::KEY_FN_F11.code()),
        (KeyCode::KEY_FN_F12, evdev::KeyCode::KEY_FN_F12.code()),
        (KeyCode::KEY_FN_1, evdev::KeyCode::KEY_FN_1.code()),
        (KeyCode::KEY_FN_2, evdev::KeyCode::KEY_FN_2.code()),
        (KeyCode::KEY_FN_D, evdev::KeyCode::KEY_FN_D.code()),
        (KeyCode::KEY_FN_E, evdev::KeyCode::KEY_FN_E.code()),
        (KeyCode::KEY_FN_F, evdev::KeyCode::KEY_FN_F.code()),
        (KeyCode::KEY_FN_S, evdev::KeyCode::KEY_FN_S.code()),
        (KeyCode::KEY_FN_B, evdev::KeyCode::KEY_FN_B.code()),
        (KeyCode::KEY_BRL_DOT1, evdev::KeyCode::KEY_BRL_DOT1.code()),
        (KeyCode::KEY_BRL_DOT2, evdev::KeyCode::KEY_BRL_DOT2.code()),
        (KeyCode::KEY_BRL_DOT3, evdev::KeyCode::KEY_BRL_DOT3.code()),
        (KeyCode::KEY_BRL_DOT4, evdev::KeyCode::KEY_BRL_DOT4.code()),
        (KeyCode::KEY_BRL_DOT5, evdev::KeyCode::KEY_BRL_DOT5.code()),
        (KeyCode::KEY_BRL_DOT6, evdev::KeyCode::KEY_BRL_DOT6.code()),
        (KeyCode::KEY_BRL_DOT7, evdev::KeyCode::KEY_BRL_DOT7.code()),
        (KeyCode::KEY_BRL_DOT8, evdev::KeyCode::KEY_BRL_DOT8.code()),
        (KeyCode::KEY_BRL_DOT9, evdev::KeyCode::KEY_BRL_DOT9.code()),
        (KeyCode::KEY_BRL_DOT10, evdev::KeyCode::KEY_BRL_DOT10.code()),
        (KeyCode::KEY_NUMERIC_0, evdev::KeyCode::KEY_NUMERIC_0.code()),
        (KeyCode::KEY_NUMERIC_1, evdev::KeyCode::KEY_NUMERIC_1.code()),
        (KeyCode::KEY_NUMERIC_2, evdev::KeyCode::KEY_NUMERIC_2.code()),
        (KeyCode::KEY_NUMERIC_3, evdev::KeyCode::KEY_NUMERIC_3.code()),
        (KeyCode::KEY_NUMERIC_4, evdev::KeyCode::KEY_NUMERIC_4.code()),
        (KeyCode::KEY_NUMERIC_5, evdev::KeyCode::KEY_NUMERIC_5.code()),
        (KeyCode::KEY_NUMERIC_6, evdev::KeyCode::KEY_NUMERIC_6.code()),
        (KeyCode::KEY_NUMERIC_7, evdev::KeyCode::KEY_NUMERIC_7.code()),
        (KeyCode::KEY_NUMERIC_8, evdev::KeyCode::KEY_NUMERIC_8.code()),
        (KeyCode::KEY_NUMERIC_9, evdev::KeyCode::KEY_NUMERIC_9.code()),
        (
            KeyCode::KEY_NUMERIC_STAR,
            evdev::KeyCode::KEY_NUMERIC_STAR.code(),
        ),
        (
            KeyCode::KEY_NUMERIC_POUND,
            evdev::KeyCode::KEY_NUMERIC_POUND.code(),
        ),
        (KeyCode::KEY_NUMERIC_A, evdev::KeyCode::KEY_NUMERIC_A.code()),
        (KeyCode::KEY_NUMERIC_B, evdev::KeyCode::KEY_NUMERIC_B.code()),
        (KeyCode::KEY_NUMERIC_C, evdev::KeyCode::KEY_NUMERIC_C.code()),
        (KeyCode::KEY_NUMERIC_D, evdev::KeyCode::KEY_NUMERIC_D.code()),
        (
            KeyCode::KEY_CAMERA_FOCUS,
            evdev::KeyCode::KEY_CAMERA_FOCUS.code(),
        ),
        (
            KeyCode::KEY_WPS_BUTTON,
            evdev::KeyCode::KEY_WPS_BUTTON.code(),
        ),
        (
            KeyCode::KEY_TOUCHPAD_TOGGLE,
            evdev::KeyCode::KEY_TOUCHPAD_TOGGLE.code(),
        ),
        (
            KeyCode::KEY_TOUCHPAD_ON,
            evdev::KeyCode::KEY_TOUCHPAD_ON.code(),
        ),
        (
            KeyCode::KEY_TOUCHPAD_OFF,
            evdev::KeyCode::KEY_TOUCHPAD_OFF.code(),
        ),
        (
            KeyCode::KEY_CAMERA_ZOOMIN,
            evdev::KeyCode::KEY_CAMERA_ZOOMIN.code(),
        ),
        (
            KeyCode::KEY_CAMERA_ZOOMOUT,
            evdev::KeyCode::KEY_CAMERA_ZOOMOUT.code(),
        ),
        (KeyCode::KEY_CAMERA_UP, evdev::KeyCode::KEY_CAMERA_UP.code()),
        (
            KeyCode::KEY_CAMERA_DOWN,
            evdev::KeyCode::KEY_CAMERA_DOWN.code(),
        ),
        (
            KeyCode::KEY_CAMERA_LEFT,
            evdev::KeyCode::KEY_CAMERA_LEFT.code(),
        ),
        (
            KeyCode::KEY_CAMERA_RIGHT,
            evdev::KeyCode::KEY_CAMERA_RIGHT.code(),
        ),
        (
            KeyCode::KEY_ATTENDANT_ON,
            evdev::KeyCode::KEY_ATTENDANT_ON.code(),
        ),
        (
            KeyCode::KEY_ATTENDANT_OFF,
            evdev::KeyCode::KEY_ATTENDANT_OFF.code(),
        ),
        (
            KeyCode::KEY_ATTENDANT_TOGGLE,
            evdev::KeyCode::KEY_ATTENDANT_TOGGLE.code(),
        ),
        (
            KeyCode::KEY_LIGHTS_TOGGLE,
            evdev::KeyCode::KEY_LIGHTS_TOGGLE.code(),
        ),
        (KeyCode::BTN_DPAD_UP, evdev::KeyCode::BTN_DPAD_UP.code()),
        (KeyCode::BTN_DPAD_DOWN, evdev::KeyCode::BTN_DPAD_DOWN.code()),
        (KeyCode::BTN_DPAD_LEFT, evdev::KeyCode::BTN_DPAD_LEFT.code()),
        (
            KeyCode::BTN_DPAD_RIGHT,
            evdev::KeyCode::BTN_DPAD_RIGHT.code(),
        ),
        (
            KeyCode::KEY_ALS_TOGGLE,
            evdev::KeyCode::KEY_ALS_TOGGLE.code(),
        ),
        (
            KeyCode::KEY_BUTTONCONFIG,
            evdev::KeyCode::KEY_BUTTONCONFIG.code(),
        ),
        (
            KeyCode::KEY_TASKMANAGER,
            evdev::KeyCode::KEY_TASKMANAGER.code(),
        ),
        (KeyCode::KEY_JOURNAL, evdev::KeyCode::KEY_JOURNAL.code()),
        (
            KeyCode::KEY_CONTROLPANEL,
            evdev::KeyCode::KEY_CONTROLPANEL.code(),
        ),
        (KeyCode::KEY_APPSELECT, evdev::KeyCode::KEY_APPSELECT.code()),
        (
            KeyCode::KEY_SCREENSAVER,
            evdev::KeyCode::KEY_SCREENSAVER.code(),
        ),
        (
            KeyCode::KEY_VOICECOMMAND,
            evdev::KeyCode::KEY_VOICECOMMAND.code(),
        ),
        (KeyCode::KEY_ASSISTANT, evdev::KeyCode::KEY_ASSISTANT.code()),
        (
            KeyCode::KEY_KBD_LAYOUT_NEXT,
            evdev::KeyCode::KEY_KBD_LAYOUT_NEXT.code(),
        ),
        (
            KeyCode::KEY_BRIGHTNESS_MIN,
            evdev::KeyCode::KEY_BRIGHTNESS_MIN.code(),
        ),
        (
            KeyCode::KEY_BRIGHTNESS_MAX,
            evdev::KeyCode::KEY_BRIGHTNESS_MAX.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_PREV,
            evdev::KeyCode::KEY_KBDINPUTASSIST_PREV.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_NEXT,
            evdev::KeyCode::KEY_KBDINPUTASSIST_NEXT.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_PREVGROUP,
            evdev::KeyCode::KEY_KBDINPUTASSIST_PREVGROUP.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_NEXTGROUP,
            evdev::KeyCode::KEY_KBDINPUTASSIST_NEXTGROUP.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_ACCEPT,
            evdev::KeyCode::KEY_KBDINPUTASSIST_ACCEPT.code(),
        ),
        (
            KeyCode::KEY_KBDINPUTASSIST_CANCEL,
            evdev::KeyCode::KEY_KBDINPUTASSIST_CANCEL.code(),
        ),
        (KeyCode::KEY_RIGHT_UP, evdev::KeyCode::KEY_RIGHT_UP.code()),
        (
            KeyCode::KEY_RIGHT_DOWN,
            evdev::KeyCode::KEY_RIGHT_DOWN.code(),
        ),
        (KeyCode::KEY_LEFT_UP, evdev::KeyCode::KEY_LEFT_UP.code()),
        (KeyCode::KEY_LEFT_DOWN, evdev::KeyCode::KEY_LEFT_DOWN.code()),
        (KeyCode::KEY_ROOT_MENU, evdev::KeyCode::KEY_ROOT_MENU.code()),
        (
            KeyCode::KEY_MEDIA_TOP_MENU,
            evdev::KeyCode::KEY_MEDIA_TOP_MENU.code(),
        ),
        (
            KeyCode::KEY_NUMERIC_11,
            evdev::KeyCode::KEY_NUMERIC_11.code(),
        ),
        (
            KeyCode::KEY_NUMERIC_12,
            evdev::KeyCode::KEY_NUMERIC_12.code(),
        ),
        (
            KeyCode::KEY_AUDIO_DESC,
            evdev::KeyCode::KEY_AUDIO_DESC.code(),
        ),
        (KeyCode::KEY_3D_MODE, evdev::KeyCode::KEY_3D_MODE.code()),
        (
            KeyCode::KEY_NEXT_FAVORITE,
            evdev::KeyCode::KEY_NEXT_FAVORITE.code(),
        ),
        (
            KeyCode::KEY_STOP_RECORD,
            evdev::KeyCode::KEY_STOP_RECORD.code(),
        ),
        (
            KeyCode::KEY_PAUSE_RECORD,
            evdev::KeyCode::KEY_PAUSE_RECORD.code(),
        ),
        (KeyCode::KEY_VOD, evdev::KeyCode::KEY_VOD.code()),
        (KeyCode::KEY_UNMUTE, evdev::KeyCode::KEY_UNMUTE.code()),
        (
            KeyCode::KEY_FASTREVERSE,
            evdev::KeyCode::KEY_FASTREVERSE.code(),
        ),
        (
            KeyCode::KEY_SLOWREVERSE,
            evdev::KeyCode::KEY_SLOWREVERSE.code(),
        ),
        (KeyCode::KEY_DATA, evdev::KeyCode::KEY_DATA.code()),
        (
            KeyCode::KEY_ONSCREEN_KEYBOARD,
            evdev::KeyCode::KEY_ONSCREEN_KEYBOARD.code(),
        ),
        (
            KeyCode::KEY_PRIVACY_SCREEN_TOGGLE,
            evdev::KeyCode::KEY_PRIVACY_SCREEN_TOGGLE.code(),
        ),
        (
            KeyCode::KEY_SELECTIVE_SCREENSHOT,
            evdev::KeyCode::KEY_SELECTIVE_SCREENSHOT.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY1,
            evdev::KeyCode::BTN_TRIGGER_HAPPY1.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY2,
            evdev::KeyCode::BTN_TRIGGER_HAPPY2.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY3,
            evdev::KeyCode::BTN_TRIGGER_HAPPY3.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY4,
            evdev::KeyCode::BTN_TRIGGER_HAPPY4.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY5,
            evdev::KeyCode::BTN_TRIGGER_HAPPY5.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY6,
            evdev::KeyCode::BTN_TRIGGER_HAPPY6.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY7,
            evdev::KeyCode::BTN_TRIGGER_HAPPY7.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY8,
            evdev::KeyCode::BTN_TRIGGER_HAPPY8.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY9,
            evdev::KeyCode::BTN_TRIGGER_HAPPY9.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY10,
            evdev::KeyCode::BTN_TRIGGER_HAPPY10.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY11,
            evdev::KeyCode::BTN_TRIGGER_HAPPY11.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY12,
            evdev::KeyCode::BTN_TRIGGER_HAPPY12.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY13,
            evdev::KeyCode::BTN_TRIGGER_HAPPY13.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY14,
            evdev::KeyCode::BTN_TRIGGER_HAPPY14.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY15,
            evdev::KeyCode::BTN_TRIGGER_HAPPY15.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY16,
            evdev::KeyCode::BTN_TRIGGER_HAPPY16.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY17,
            evdev::KeyCode::BTN_TRIGGER_HAPPY17.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY18,
            evdev::KeyCode::BTN_TRIGGER_HAPPY18.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY19,
            evdev::KeyCode::BTN_TRIGGER_HAPPY19.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY20,
            evdev::KeyCode::BTN_TRIGGER_HAPPY20.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY21,
            evdev::KeyCode::BTN_TRIGGER_HAPPY21.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY22,
            evdev::KeyCode::BTN_TRIGGER_HAPPY22.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY23,
            evdev::KeyCode::BTN_TRIGGER_HAPPY23.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY24,
            evdev::KeyCode::BTN_TRIGGER_HAPPY24.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY25,
            evdev::KeyCode::BTN_TRIGGER_HAPPY25.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY26,
            evdev::KeyCode::BTN_TRIGGER_HAPPY26.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY27,
            evdev::KeyCode::BTN_TRIGGER_HAPPY27.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY28,
            evdev::KeyCode::BTN_TRIGGER_HAPPY28.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY29,
            evdev::KeyCode::BTN_TRIGGER_HAPPY29.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY30,
            evdev::KeyCode::BTN_TRIGGER_HAPPY30.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY31,
            evdev::KeyCode::BTN_TRIGGER_HAPPY31.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY32,
            evdev::KeyCode::BTN_TRIGGER_HAPPY32.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY33,
            evdev::KeyCode::BTN_TRIGGER_HAPPY33.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY34,
            evdev::KeyCode::BTN_TRIGGER_HAPPY34.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY35,
            evdev::KeyCode::BTN_TRIGGER_HAPPY35.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY36,
            evdev::KeyCode::BTN_TRIGGER_HAPPY36.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY37,
            evdev::KeyCode::BTN_TRIGGER_HAPPY37.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY38,
            evdev::KeyCode::BTN_TRIGGER_HAPPY38.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY39,
            evdev::KeyCode::BTN_TRIGGER_HAPPY39.code(),
        ),
        (
            KeyCode::BTN_TRIGGER_HAPPY40,
            evdev::KeyCode::BTN_TRIGGER_HAPPY40.code(),
        ),
    ];
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn portable_codes_match_evdev() {
        for (portable, evdev) in super::portable::ALL {
            assert_eq!(portable.code(), *evdev);
        }
    }
}
