//! Every GRF resource path the client reads, in one place.
//!
//! Values are GRF entry names: forward slashes, Korean folder names spelled the
//! way the archive stores them. Paths built at runtime are functions here too,
//! so no `data/` literal needs to live anywhere else.

/// Declares path constants and the module's `ALL` slice from one list, so the
/// two cannot drift.
#[macro_export]
macro_rules! paths {
    ($($(#[$attr:meta])* $name:ident = $value:literal;)*) => {
        $($(#[$attr])* pub const $name: &str = $value;)*

        /// Every path declared in this module.
        pub const ALL: &[&str] = &[$($value),*];
    };
}

/// Every path the registry states outright. Paths assembled at runtime are not
/// here — those come from the builder functions.
pub fn all_static_paths() -> Vec<&'static str> {
    const MODULES: &[&[&str]] = &[
        dir::ALL,
        font::ALL,
        grf::ALL,
        lua::ALL,
        sprite::ALL,
        sprite::effect::ALL,
        sprite::item::ALL,
        sprite::monster::ALL,
        sprite::pet_accessory::ALL,
        sprite::player::ALL,
        table::ALL,
        texture::ALL,
        texture::effect::ALL,
        ui::ALL,
        ui::basic::ALL,
        ui::basic::seekparty::ALL,
        ui::illust::ALL,
        ui::item::ALL,
        ui::login::ALL,
        ui::minimap::ALL,
    ];
    MODULES.iter().flat_map(|m| m.iter().copied()).collect()
}

/// Directory prefixes, for listing or stripping — not resources themselves.
pub mod dir {
    crate::paths! {
        DATA = "data/";
        SPRITE_ACCESSORY = "data/sprite/악세사리/";
        SPRITE_MONSTER = "data/sprite/몬스터/";
        SPRITE_NPC = "data/sprite/npc/";
        SPRITE_PLAYER = "data/sprite/인간족/";
        SPRITE_SHIELD = "data/sprite/방패/";
        STR_EFFECT = "data/texture/effect/";
        TEXTURE = "data/texture/";
        UI_TEXTURE = "data/texture/유저인터페이스/";
        UI_TEXTURE_EN = "data/texture/userinterface/";
    }
}

/// TrueType fonts shipped in the archive.
pub mod font {
    crate::paths! {
        NANUM_BARUN_GOTHIC = "data/Font/NanumBarunGothic.ttf";
        NANUM_BARUN_GOTHIC_BOLD = "data/Font/NanumBarunGothicBold.ttf";
    }
}

/// GRF archives.
pub mod grf {
    crate::paths! {
        DEFAULT_ARCHIVE = "data/data.grf";
    }
}

/// Head-attachment offset tables, one per job.
pub mod imf {
    pub fn of(base: &str) -> String {
        format!("data/imf/{base}.imf")
    }

    pub fn for_job(job: &str, sex: &str) -> String {
        format!("data/imf/{job}_{sex}.imf")
    }
}

/// Lua/Lub identity tables. `.lua` and `.lub` are alternate encodings of
/// the same table; readers try one then the other.
pub mod lua {
    crate::paths! {
        ACCESSORY_ID_514_LUB = "data/luafiles514/lua files/datainfo/accessoryid.lub";
        ACCESSORY_ID_LUA = "data/lua files/datainfo/accessoryid.lua";
        ACCESSORY_ID_LUB = "data/lua files/datainfo/accessoryid.lub";
        ACCESSORY_NAME_514_LUB = "data/luafiles514/lua files/datainfo/accname.lub";
        ACCESSORY_NAME_LUA = "data/lua files/datainfo/accname.lua";
        ACCESSORY_NAME_LUB = "data/lua files/datainfo/accname.lub";
        JOB_IDENTITY_514_LUB = "data/luafiles514/lua files/datainfo/jobidentity.lub";
        JOB_NAME_514_LUB = "data/luafiles514/lua files/datainfo/jobname.lub";
        JOB_NAME_LUB = "data/lua files/datainfo/jobname.lub";
        JOB_IDENTITY_LUA = "data/lua files/datainfo/jobidentity.lua";
        JOB_IDENTITY_LUB = "data/lua files/datainfo/jobidentity.lub";
        NPC_IDENTITY_514_LUB = "data/luafiles514/lua files/datainfo/npcidentity.lub";
        NPC_IDENTITY_LUA = "data/lua files/datainfo/npcidentity.lua";
        NPC_IDENTITY_LUB = "data/lua files/datainfo/npcidentity.lub";
    }
}

/// Map geometry: one `.rsw`/`.gnd`/`.gat` triple per map.
pub mod map {
    pub fn rsw(map_name: &str) -> String {
        format!("data/{map_name}.rsw")
    }

    pub fn gnd(map_name: &str) -> String {
        format!("data/{map_name}.gnd")
    }

    pub fn gat(map_name: &str) -> String {
        format!("data/{map_name}.gat")
    }

    /// For names already carrying their extension, as RSW headers store them.
    pub fn file(file_name: &str) -> String {
        format!("data/{file_name}")
    }
}

/// 3D monster models and their animation clips.
pub mod model {
    pub fn mob(file_name: &str) -> String {
        format!("data/model/3dmob/{file_name}")
    }

    pub fn mob_animation(bone_type: u32, suffix: &str) -> String {
        format!("data/model/3dmob_bone/{bone_type}_{suffix}.gr2")
    }
}

/// Sprite recolour palettes.
pub mod palette {
    pub fn head(head_name: &str, sex: &str, palette_id: u16) -> String {
        format!("data/palette/머리/머리{head_name}_{sex}_{palette_id}.pal")
    }

    pub fn body(job: &str, sex: &str, palette_id: u16) -> String {
        format!("data/palette/몸/{job}_{sex}_{palette_id}.pal")
    }
}

/// BGM tracks and sound effects.
pub mod sound {
    pub fn bgm(track: &str) -> String {
        format!("data/wav/bgm/{track}")
    }

    pub fn sfx(name: &str) -> String {
        format!("data/wav/{name}")
    }
}

/// Sprites outside the actor trees.
pub mod sprite {
    crate::paths! {
        CURSORS_ACT = "data/sprite/cursors.act";
        CURSORS_SPR = "data/sprite/cursors.spr";
        SHADOW = "data/sprite/shadow";
        SHADOW_ACT = "data/sprite/shadow.act";
        SHADOW_SPR = "data/sprite/shadow.spr";
        SLOTMACHINE_ACT = "data/sprite/slotmachine.act";
        SLOTMACHINE_SPR = "data/sprite/slotmachine.spr";
    }

    /// Headgear sprites, drawn per sex.
    pub mod accessory {
        pub fn of(sex: &str, suffix: &str) -> String {
            format!("data/sprite/악세사리/{sex}/{sex}{suffix}")
        }
    }

    /// The `이팩트` tree — visual-effect sprites.
    pub mod effect {
        crate::paths! {
            ANGEL = "data/sprite/이팩트/천사";
            ANGEL_FEATHER = "data/sprite/이팩트/천사날개깃털";
            ANGEL_WINGS = "data/sprite/이팩트/천사날개";
            AQUA_BENEDICTA = "data/sprite/이팩트/성수뜨기";
            BAT = "data/sprite/이팩트/박쥐";
            BLESSING = "data/sprite/이팩트/축복";
            BLOODLUST = "data/sprite/이팩트/블러드러스트";
            CASTLING = "data/sprite/이팩트/캐슬링";
            CCONFINE = "data/sprite/이팩트/cconfine";
            CHIMNEY_SMOKE = "data/sprite/이팩트/굴뚝연기";
            CHRISTMAS = "data/sprite/이팩트/크리스마스";
            DARKBREATH = "data/sprite/이팩트/darkbreath";
            DEMONSTRATION = "data/sprite/이팩트/데몬스트레이션";
            DESPERADO = "data/sprite/이팩트/데스페라도";
            EARTHQUAKE = "data/sprite/이팩트/어스퀘이크";
            EF_SNOW = "data/sprite/이팩트/ef_snow";
            EMOTION_ACT = "data/sprite/이팩트/emotion.act";
            EMOTION_SPR = "data/sprite/이팩트/emotion.spr";
            FALCON = "data/sprite/이팩트/매";
            FALCON2 = "data/sprite/이팩트/매2";
            FAST = "data/sprite/이팩트/fast";
            FIREBALL = "data/sprite/이팩트/fireball";
            FIREWORK_BIRTHDAY = "data/sprite/이팩트/폭죽_생일";
            FIREWORK_CHRISTMAS = "data/sprite/이팩트/폭죽_크리스마스";
            FIREWORK_LOVE = "data/sprite/이팩트/폭죽_러브";
            FIREWORK_VALENTINE = "data/sprite/이팩트/폭죽_발렌타인";
            FIREWORK_WHITE_DAY = "data/sprite/이팩트/폭죽_화이트데이";
            FVOICE = "data/sprite/이팩트/fvoice";
            GHOST = "data/sprite/이팩트/유령";
            HANBOK_ANGEL_BODY = "data/sprite/이팩트/한복천사(본체)";
            HANBOK_ANGEL_WINGS = "data/sprite/이팩트/한복천사(날개)";
            ICE_BLOCK = "data/sprite/이팩트/얼음땡";
            ISSEN = "data/sprite/이팩트/일섬";
            ITEM_CLOUD = "data/sprite/이팩트/item_cloud";
            ITEM_CURSE = "data/sprite/이팩트/item_curse";
            ITEM_RAIN = "data/sprite/이팩트/item_rain";
            ITEM_THUNDER = "data/sprite/이팩트/item_thunder";
            ITEM_ZZZ = "data/sprite/이팩트/item_zzz";
            KAEN = "data/sprite/이팩트/화염진";
            KASUMIKIRI = "data/sprite/이팩트/안개베기";
            KIRIKAGE = "data/sprite/이팩트/그림자베기";
            MAGICAL_BULLET = "data/sprite/이팩트/매지컬불릿";
            MAPLE_LEAF = "data/sprite/이팩트/단풍";
            MSG_ACT = "data/sprite/이팩트/msg.act";
            MSG_SPR = "data/sprite/이팩트/msg.spr";
            M_EF01 = "data/sprite/이팩트/m_ef01";
            M_EF02 = "data/sprite/이팩트/m_ef02";
            M_EF03 = "data/sprite/이팩트/m_ef03";
            M_EF04 = "data/sprite/이팩트/m_ef04";
            M_EF05 = "data/sprite/이팩트/m_ef05";
            M_EF06 = "data/sprite/이팩트/m_ef06";
            M_EF07 = "data/sprite/이팩트/m_ef07";
            NUMBER_ACT = "data/sprite/이팩트/숫자.act";
            NUMBER_SPR = "data/sprite/이팩트/숫자.spr";
            ORCFACE = "data/sprite/이팩트/orcface";
            PARTICLE1 = "data/sprite/이팩트/particle1";
            PARTICLE2 = "data/sprite/이팩트/particle2";
            PARTICLE3 = "data/sprite/이팩트/particle3";
            PARTICLE4 = "data/sprite/이팩트/particle4";
            PARTICLE5 = "data/sprite/이팩트/particle5";
            PARTICLE6 = "data/sprite/이팩트/particle6";
            PARTICLE7 = "data/sprite/이팩트/particle7";
            POISONHIT = "data/sprite/이팩트/poisonhit";
            RANKFONT_ACT = "data/sprite/이팩트/rankfont.act";
            RANKFONT_SPR = "data/sprite/이팩트/rankfont.spr";
            RAPID_SHOWER = "data/sprite/이팩트/래피드샤워";
            SAKURA01 = "data/sprite/이팩트/sakura01";
            SIGHT = "data/sprite/이팩트/sight";
            SMOKE = "data/sprite/이팩트/smoke";
            SPEAR = "data/sprite/이팩트/창";
            SPREAD_ATTACK = "data/sprite/이팩트/스프레드";
            STATUS_CURSE = "data/sprite/이팩트/status-curse";
            STATUS_SLEEP = "data/sprite/이팩트/status-sleep";
            STATUS_STUN = "data/sprite/이팩트/status-stun";
            STOP = "data/sprite/이팩트/스톱";
            TATAMI_FLIP = "data/sprite/이팩트/다다미 뒤집기";
            TIMEFONT_ACT = "data/sprite/이팩트/timefont.act";
            TIMEFONT_SPR = "data/sprite/이팩트/timefont.spr";
            TORCH_01 = "data/sprite/이팩트/torch_01";
            TRACKING = "data/sprite/이팩트/트래킹";
            TRIPLE_ACTION = "data/sprite/이팩트/트리플액션";
            VALLENTINE = "data/sprite/이팩트/vallentine";
            WATERBALL = "data/sprite/이팩트/waterball";
            WINK = "data/sprite/이팩트/wink";
        }

        /// Design 0 is the Merchant's own cart, 1.. the purchasable ones.
        pub fn cart(design: u8) -> String {
            match design {
                0 => "data/sprite/이팩트/슈노손수레".to_string(),
                1 => "data/sprite/이팩트/손수레".to_string(),
                n => format!("data/sprite/이팩트/손수레{}", n - 1),
            }
        }
    }

    /// Homunculus sprites.
    pub mod homun {
        pub fn of(name: &str) -> String {
            format!("data/sprite/homun/{name}")
        }
    }

    /// Dropped-item sprites.
    pub mod item {
        crate::paths! {
            BLIND_SPEAR = "data/sprite/아이템/블라인드스피어";
            BLUE_GEMSTONE = "data/sprite/아이템/블루젬스톤";
            FLARE_SPEAR = "data/sprite/아이템/플레어스피어";
            FREEZING_SPEAR = "data/sprite/아이템/프리징스피어";
            LIGHTNING_SPEAR = "data/sprite/아이템/라이트닝스피어";
            POISON_SPEAR = "data/sprite/아이템/포이즌스피어";
            RED_GEMSTONE = "data/sprite/아이템/레드젬스톤";
            YELLOW_GEMSTONE = "data/sprite/아이템/옐로우젬스톤";
        }

        pub fn of(name: &str) -> String {
            format!("data/sprite/아이템/{name}")
        }
    }

    /// Monster sprites.
    pub mod monster {
        crate::paths! {
            SKEL_ARCHER_ARROW = "data/sprite/몬스터/skel_archer_arrow";
        }

        pub fn of(name: &str) -> String {
            format!("data/sprite/몬스터/{name}")
        }
    }

    /// NPC sprites.
    pub mod npc {
        pub fn of(name: &str) -> String {
            format!("data/sprite/npc/{name}")
        }
    }

    /// Pet headgear animations, keyed by the pet mob.
    pub mod pet_accessory {
        crate::paths! {
            BACSOJIN = "data/sprite/몬스터/BACSOJIN_동그란머리장식.act";
            BAPHOMET = "data/sprite/몬스터/BAPHOMET_뼉다구모자.act";
            BON_GUN = "data/sprite/몬스터/bon_gun_영환도사검.act";
            CHOCHO = "data/sprite/몬스터/chocho_방독면.act";
            CIVIL_SERVANT = "data/sprite/몬스터/CIVIL_SERVANT_금빛귀걸이.act";
            DESERT_WOLF_B = "data/sprite/몬스터/DESERT_WOLF_B_우주복머리.act";
            DEVIRUCHI = "data/sprite/몬스터/DEVIRUCHI_젖꼭지.act";
            DOKEBI = "data/sprite/몬스터/DOKEBI_아후로머리.act";
            DULLAHAN = "data/sprite/몬스터/DULLAHAN_죽음의고리.act";
            GOBLIN_LEADER = "data/sprite/몬스터/GOBLIN_LEADER_멋진휘장.act";
            GOLEM = "data/sprite/몬스터/GOLEM_태엽.act";
            IMP = "data/sprite/몬스터/IMP_뿔보호대.act";
            INCUBUS = "data/sprite/몬스터/INCUBUS_무도회가면.act";
            ISIS = "data/sprite/몬스터/isis_클레오파트라머리띠.act";
            LEAF_CAT = "data/sprite/몬스터/LEAF_CAT_초록복주머니.act";
            LOLI_RURI = "data/sprite/몬스터/LOLI_RURI_패션안경.act";
            LUNATIC = "data/sprite/몬스터/lunatic_리본.act";
            MARIONETTE = "data/sprite/몬스터/MARIONETTE_별모양머리띠.act";
            MEDUSA = "data/sprite/몬스터/MEDUSA_여왕의코로넷.act";
            MIYABI_NINGYO = "data/sprite/몬스터/MIYABI_NINGYO_여름부채.act";
            MUNAK = "data/sprite/몬스터/munak_요술봉.act";
            NIGHTMARE_TERROR = "data/sprite/몬스터/NIGHTMARE_TERROR_지옥의뿔.act";
            ORK_WARRIOR = "data/sprite/몬스터/ork_warrior_꽃.act";
            PECOPECO = "data/sprite/몬스터/pecopeco_냄비.act";
            PETIT = "data/sprite/몬스터/PETIT_별.act";
            PICKY = "data/sprite/몬스터/picky_알껍질.act";
            PORING = "data/sprite/몬스터/poring_책가방.act";
            ROCKER = "data/sprite/몬스터/rocker_메뚜기안경.act";
            SAVAGE_BABE = "data/sprite/몬스터/savage_babe_레이스.act";
            SHINOBI = "data/sprite/몬스터/SHINOBI_두루마기용술.act";
            SMOKIE = "data/sprite/몬스터/smokie_머플러.act";
            SOHEE = "data/sprite/몬스터/SOHEE_방울.act";
            SPORE = "data/sprite/몬스터/spore_원주민치마.act";
            STONE_SHOOTER = "data/sprite/몬스터/STONE_SHOOTER_아프로헤어.act";
            SUCCUBUS = "data/sprite/몬스터/SUCCUBUS_검은나비가면.act";
            WHISPER = "data/sprite/몬스터/WHISPER_영혼고리_.act";
            WICKED_NYMPH = "data/sprite/몬스터/WICKED_NYMPH_옥노리개.act";
            YOYO = "data/sprite/몬스터/yoyo_머리띠.act";
        }
    }

    /// Player body, head and weapon sprites.
    pub mod player {
        crate::paths! {
            GRAND_PECO_CRUSADER_FEMALE = "data/sprite/인간족/몸통/여/신페코크루세이더_여";
            GRAND_PECO_CRUSADER_MALE = "data/sprite/인간족/몸통/남/신페코크루세이더_남";
            LORD_PECO_MALE = "data/sprite/인간족/몸통/남/로드페코_남";
            PECO_KNIGHT_MALE = "data/sprite/인간족/몸통/남/페코페코_기사_남";
            PECO_PALADIN_MALE = "data/sprite/인간족/몸통/남/페코팔라딘_남";
        }

        pub fn body(job: &str, sex: &str) -> String {
            format!("data/sprite/인간족/몸통/{sex}/{job}_{sex}")
        }

        pub fn head(head_name: &str, sex: &str) -> String {
            format!("data/sprite/인간족/머리통/{sex}/{head_name}_{sex}")
        }

        pub fn weapon(job: &str, sex: &str, suffix: &str) -> String {
            format!("data/sprite/인간족/{job}/{job}_{sex}{suffix}")
        }

        /// Weapon art keyed by item id, preferred over [`weapon`] when present.
        pub fn weapon_by_item(job: &str, sex: &str, item_id: u16) -> String {
            format!("data/sprite/인간족/{job}/{job}_{sex}_{item_id}")
        }

        pub fn gm_body(sex: &str) -> String {
            format!("data/sprite/인간족/몸통/{sex}/운영자_{sex}")
        }

        pub fn gm_weapon(sex: &str) -> String {
            format!("data/sprite/인간족/운영자/운영자_{sex}_검")
        }

        /// The name table already carries the sex/type sub-path.
        pub fn mercenary_body(name: &str) -> String {
            format!("data/sprite/인간족/몸통/{name}")
        }

        pub fn mercenary_weapon(base: &str, weapon_char: char) -> String {
            format!("data/sprite/인간족/용병/{base}_{weapon_char}")
        }
    }

    /// Shield sprites, drawn per job and sex.
    pub mod shield {
        pub fn of(job: &str, sex: &str, shield: impl std::fmt::Display) -> String {
            format!("data/sprite/방패/{job}/{job}_{sex}_{shield}")
        }
    }
}

/// Flat `data/*.txt` lookup tables.
pub mod table {
    crate::paths! {
        CARD_ILLUSTRATION_NAME = "data/num2cardillustnametable.txt";
        CARD_POSTFIX_NAME = "data/cardpostfixnametable.txt";
        CARD_PREFIX_NAME = "data/cardprefixnametable.txt";
        FOG_PARAMETER = "data/fogparametertable.txt";
        IDENTIFIED_ITEM_DESC = "data/idnum2itemdesctable.txt";
        IDENTIFIED_ITEM_NAME = "data/idnum2itemdisplaynametable.txt";
        IDENTIFIED_ITEM_RESOURCE = "data/idnum2itemresnametable.txt";
        INDOOR_RSW = "data/indoorrswtable.txt";
        ITEM_SLOT_COUNT = "data/itemslotcounttable.txt";
        MAP_NAME = "data/mapnametable.txt";
        MAP_POSITION = "data/mappostable.txt";
        MP3_NAME = "data/mp3nametable.txt";
        MSG_STRING = "data/msgstringtable.txt";
        PET_TALK = "data/pettalktable.xml";
        QUEST_DISPLAY = "data/questid2display.txt";
        RES_NAME = "data/resnametable.txt";
        SKILL_DESC = "data/skilldesctable.txt";
        SKILL_NAME = "data/skillnametable.txt";
        SKILL_SP_AMOUNT = "data/leveluseskillspamount.txt";
        SKILL_TREE = "data/skilltreeview.txt";
        UNIDENTIFIED_ITEM_DESC = "data/num2itemdesctable.txt";
        UNIDENTIFIED_ITEM_NAME = "data/num2itemdisplaynametable.txt";
        UNIDENTIFIED_ITEM_RESOURCE = "data/num2itemresnametable.txt";
    }

    pub fn book(item_id: impl std::fmt::Display) -> String {
        format!("data/book/{item_id}.txt")
    }
}

/// Textures outside the UI tree.
pub mod texture {
    crate::paths! {
        GRID = "data/texture/grid.tga";
    }

    pub fn named(name: &str) -> String {
        format!("data/texture/{name}")
    }

    pub fn water(water_type: i32, frame: usize) -> String {
        format!("data/texture/워터/water{water_type}{frame:02}.jpg")
    }

    /// Effect textures and `.str` animation scripts.
    pub mod effect {
        crate::paths! {
            ALPHABET = "data/texture/effect/alpabet.bmp";
        }

        pub fn named(name: &str) -> String {
            format!("data/texture/effect/{name}")
        }

        pub fn str_file(name: &str) -> String {
            format!("data/texture/effect/{name}.str")
        }
    }
}

/// The `유저인터페이스` tree — every window, button and icon texture.
pub mod ui {
    crate::paths! {
        BTN_CANCEL = "data/texture/유저인터페이스/btn_cancel.bmp";
        BTN_CANCEL_A = "data/texture/유저인터페이스/btn_cancel_a.bmp";
        BTN_CANCEL_B = "data/texture/유저인터페이스/btn_cancel_b.bmp";
        BTN_EXCHANGE_DIS = "data/texture/유저인터페이스/btn_exchange_dis.bmp";
        BTN_MAKE = "data/texture/유저인터페이스/btn_make.bmp";
        BTN_MAKE_A = "data/texture/유저인터페이스/btn_make_a.bmp";
        BTN_MAKE_B = "data/texture/유저인터페이스/btn_make_b.bmp";
        BTN_OK = "data/texture/유저인터페이스/btn_ok.bmp";
        BTN_OK_A = "data/texture/유저인터페이스/btn_ok_a.bmp";
        BTN_OK_B = "data/texture/유저인터페이스/btn_ok_b.bmp";
        BTN_OK_DIS = "data/texture/유저인터페이스/btn_ok_dis.bmp";
        CHECKBOX_0 = "data/texture/유저인터페이스/checkbox_0.bmp";
        CHECKBOX_1 = "data/texture/유저인터페이스/checkbox_1.bmp";
        RAG_TITLE = "data/texture/유저인터페이스/rag_title.bmp";
        RAG_TITLE2 = "data/texture/유저인터페이스/rag_title2.bmp";
        RAG_TITLE3 = "data/texture/유저인터페이스/rag_title3.bmp";
        SCROLL0UP = "data/texture/유저인터페이스/scroll0up.bmp";
        SCROLL1LEFT = "data/texture/유저인터페이스/scroll1left.bmp";
        SCROLL1RIGHT = "data/texture/유저인터페이스/scroll1right.bmp";
        WORLDMAP = "data/texture/유저인터페이스/worldmap.bmp";
    }

    /// Shared window chrome and the in-game HUD.
    pub mod basic {
        crate::paths! {
            BTNBAR_MID2 = "data/texture/유저인터페이스/basic_interface/btnbar_mid2.bmp";
            GRP_LEADER = "data/texture/유저인터페이스/basic_interface/grp_leader.bmp";
            GRP_ONLINE = "data/texture/유저인터페이스/basic_interface/grp_online.bmp";
            GZEBLUE_LEFT = "data/texture/유저인터페이스/basic_interface/gzeblue_left.bmp";
            GZEBLUE_MID = "data/texture/유저인터페이스/basic_interface/gzeblue_mid.bmp";
            GZEBLUE_RIGHT = "data/texture/유저인터페이스/basic_interface/gzeblue_right.bmp";
            GZERED_LEFT = "data/texture/유저인터페이스/basic_interface/gzered_left.bmp";
            GZERED_MID = "data/texture/유저인터페이스/basic_interface/gzered_mid.bmp";
            GZERED_RIGHT = "data/texture/유저인터페이스/basic_interface/gzered_right.bmp";
            ITEMWIN_MID = "data/texture/유저인터페이스/basic_interface/itemwin_mid.bmp";
            MAILLIST1_BG = "data/texture/유저인터페이스/basic_interface/maillist1_bg.bmp";
            MAILLIST2_BG = "data/texture/유저인터페이스/basic_interface/maillist2_bg.bmp";
            SYS_BASE_OFF = "data/texture/유저인터페이스/basic_interface/sys_base_off.bmp";
            SYS_BASE_ON = "data/texture/유저인터페이스/basic_interface/sys_base_on.bmp";
            TAB_ITM_01 = "data/texture/유저인터페이스/basic_interface/tab_itm_01.bmp";
            TAB_ITM_02 = "data/texture/유저인터페이스/basic_interface/tab_itm_02.bmp";
            TAB_ITM_03 = "data/texture/유저인터페이스/basic_interface/tab_itm_03.bmp";
            TITLEBAR_MID = "data/texture/유저인터페이스/basic_interface/titlebar_mid.bmp";
        }

        /// Party-recruitment board buttons.
        pub mod seekparty {
            crate::paths! {
            }
        }
    }

    /// Card illustrations.
    pub mod cardbmp {
        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/cardbmp/{name}.bmp")
        }
    }

    /// Item collection illustrations.
    pub mod collection {
        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/collection/{name}.bmp")
        }
    }

    /// Pet illustrations, keyed by the pet mob.
    pub mod illust {
        crate::paths! {
            PET_ALICE = "data/texture/유저인터페이스/illust/펫_ALICE.bmp";
            PET_BABY_DESERT_WOLF = "data/texture/유저인터페이스/illust/펫_데져트울프새끼.bmp";
            PET_BACSOJIN = "data/texture/유저인터페이스/illust/펫_BACSOJIN.bmp";
            PET_BAPHOMET_JR = "data/texture/유저인터페이스/illust/펫_바포메트.bmp";
            PET_BONGUN = "data/texture/유저인터페이스/illust/펫_본건.bmp";
            PET_CHONCHON = "data/texture/유저인터페이스/illust/펫_촌촌.bmp";
            PET_CHRISTMAS_SNOW_RABBIT = "data/texture/유저인터페이스/illust/펫_크리스마스_눈토끼.bmp";
            PET_CHUNG_E = "data/texture/유저인터페이스/illust/펫_청이.bmp";
            PET_CIVIL_SERVANT = "data/texture/유저인터페이스/illust/펫_CIVIL_SERVANT.bmp";
            PET_DELETER = "data/texture/유저인터페이스/illust/펫_지상딜리터.bmp";
            PET_DEVIRUCHI = "data/texture/유저인터페이스/illust/펫_데비루치.bmp";
            PET_DIABOLIC = "data/texture/유저인터페이스/illust/펫_디아볼릭.bmp";
            PET_DOKEBI = "data/texture/유저인터페이스/illust/펫_도깨비.bmp";
            PET_DROPS = "data/texture/유저인터페이스/illust/펫_드롭프스.bmp";
            PET_DULLAHAN = "data/texture/유저인터페이스/illust/펫_DULLAHAN.bmp";
            PET_GOBLIN_DAGGER = "data/texture/유저인터페이스/illust/펫_고블린_단검.bmp";
            PET_GOBLIN_EVENT = "data/texture/유저인터페이스/illust/펫_고블린_이벤트.bmp";
            PET_GOBLIN_FLAIL = "data/texture/유저인터페이스/illust/펫_고블린_플레일.bmp";
            PET_GOBLIN_HAMMER = "data/texture/유저인터페이스/illust/펫_고블린_해머.bmp";
            PET_GOBLIN_LEADER = "data/texture/유저인터페이스/illust/펫_GOBLIN_LEADER.bmp";
            PET_GOLEM = "data/texture/유저인터페이스/illust/펫_GOLEM.bmp";
            PET_HUNTER_FLY = "data/texture/유저인터페이스/illust/펫_헌터플라이.bmp";
            PET_IMP = "data/texture/유저인터페이스/illust/펫_IMP.bmp";
            PET_INCUBUS = "data/texture/유저인터페이스/illust/펫_INCUBUS.bmp";
            PET_ISIS = "data/texture/유저인터페이스/illust/펫_이시스.bmp";
            PET_J_TAINI = "data/texture/유저인터페이스/illust/펫_j_taini.bmp";
            PET_LEAF_CAT = "data/texture/유저인터페이스/illust/펫_LEAF_CAT.bmp";
            PET_LOLI_RURI = "data/texture/유저인터페이스/illust/펫_LOLI_RURI.BMP";
            PET_LUNATIC = "data/texture/유저인터페이스/illust/펫_루나틱.bmp";
            PET_MARIONETTE = "data/texture/유저인터페이스/illust/펫_MARIONETTE.bmp";
            PET_MEDUSA = "data/texture/유저인터페이스/illust/펫_MEDUSA.bmp";
            PET_MIYABI_NINGYO = "data/texture/유저인터페이스/illust/펫_MIYABI_NINGYO.bmp";
            PET_MUNAK = "data/texture/유저인터페이스/illust/펫_무낙.bmp";
            PET_NIGHTMARE_TERROR = "data/texture/유저인터페이스/illust/펫_NIGHTMARE_TERROR.bmp";
            PET_ORC_WARRIOR = "data/texture/유저인터페이스/illust/펫_오크워리어.bmp";
            PET_PECOPECO = "data/texture/유저인터페이스/illust/펫_페코페코.bmp";
            PET_PETITE = "data/texture/유저인터페이스/illust/펫_쁘띠.bmp";
            PET_PICKY = "data/texture/유저인터페이스/illust/펫_픽키.bmp";
            PET_POISON_SPORE = "data/texture/유저인터페이스/illust/펫_포이즌스포아.bmp";
            PET_POPORING = "data/texture/유저인터페이스/illust/펫_포포링.bmp";
            PET_PORING = "data/texture/유저인터페이스/illust/펫_포링.bmp";
            PET_RICE_CAKE = "data/texture/유저인터페이스/illust/펫_떡_이벤트.bmp";
            PET_ROCKER = "data/texture/유저인터페이스/illust/펫_로커.bmp";
            PET_SAVAGE_BABE = "data/texture/유저인터페이스/illust/펫_세비지베베.bmp";
            PET_SHINOBI = "data/texture/유저인터페이스/illust/펫_SHINOBI.bmp";
            PET_SMOKIE = "data/texture/유저인터페이스/illust/펫_스모키.bmp";
            PET_SOHEE = "data/texture/유저인터페이스/illust/펫_소희.bmp";
            PET_SPORE = "data/texture/유저인터페이스/illust/펫_스포아.bmp";
            PET_STEEL_CHONCHON = "data/texture/유저인터페이스/illust/펫_스틸촌촌.bmp";
            PET_STONE_SHOOTER = "data/texture/유저인터페이스/illust/펫_STONE_SHOOTER.bmp";
            PET_SUCCUBUS = "data/texture/유저인터페이스/illust/펫_SUCCUBUS.bmp";
            PET_WANDERER = "data/texture/유저인터페이스/illust/펫_배회하는자.bmp";
            PET_WHISPER = "data/texture/유저인터페이스/illust/펫_WHISPER.BMP";
            PET_WICKED_NYMPH = "data/texture/유저인터페이스/illust/펫_WICKED_NYMPH.bmp";
            PET_YOYO = "data/texture/유저인터페이스/illust/펫_요요.bmp";
            PET_ZHERLTHSH = "data/texture/유저인터페이스/illust/펫_ZHERLTHSH.bmp";
        }

        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/illust/{name}.bmp")
        }
    }

    /// Inventory item icons.
    pub mod item {
        crate::paths! {
        }

        pub fn icon(name: &str) -> String {
            format!("data/texture/유저인터페이스/item/{name}.bmp")
        }
    }

    /// Login, server-select and character-create screens.
    pub mod login {
        crate::paths! {
            ARW_AGI0 = "data/texture/유저인터페이스/login_interface/arw-agi0.bmp";
            ARW_AGI1 = "data/texture/유저인터페이스/login_interface/arw-agi1.bmp";
            ARW_DEX0 = "data/texture/유저인터페이스/login_interface/arw-dex0.bmp";
            ARW_DEX1 = "data/texture/유저인터페이스/login_interface/arw-dex1.bmp";
            ARW_INT0 = "data/texture/유저인터페이스/login_interface/arw-int0.bmp";
            ARW_INT1 = "data/texture/유저인터페이스/login_interface/arw-int1.bmp";
            ARW_LUK0 = "data/texture/유저인터페이스/login_interface/arw-luk0.bmp";
            ARW_LUK1 = "data/texture/유저인터페이스/login_interface/arw-luk1.bmp";
            ARW_STR0 = "data/texture/유저인터페이스/login_interface/arw-str0.bmp";
            ARW_STR1 = "data/texture/유저인터페이스/login_interface/arw-str1.bmp";
            ARW_VIT0 = "data/texture/유저인터페이스/login_interface/arw-vit0.bmp";
            ARW_VIT1 = "data/texture/유저인터페이스/login_interface/arw-vit1.bmp";
            NAME_EDIT = "data/texture/유저인터페이스/login_interface/name-edit.bmp";
            WIN_MAKE = "data/texture/유저인터페이스/login_interface/win_make.bmp";
            WIN_MAKE2 = "data/texture/유저인터페이스/login_interface/win_make2.bmp";
        }
    }

    /// Minimap frame and per-map minimap images.
    pub mod minimap {
        crate::paths! {
            PRONTERA = "data/texture/유저인터페이스/map/prontera.bmp";
        }

        pub fn of(map_name: &str) -> String {
            format!("data/texture/유저인터페이스/map/{map_name}.bmp")
        }
    }
}

#[cfg(test)]
mod tests {
    /// `all_static_paths` lists modules by hand, so a new module can be missed.
    /// Each declared path is one assignment line inside a `paths!` block.
    #[test]
    fn every_declared_path_is_enumerated() {
        let declared = include_str!("lib.rs")
            .lines()
            .filter(|l| l.contains(" = \"data/"))
            .count();
        let enumerated = super::all_static_paths();
        assert_eq!(
            declared,
            enumerated.len(),
            "a paths! module is missing from all_static_paths()"
        );

        let mut sorted = enumerated.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), enumerated.len(), "duplicate path in registry");
    }
}
