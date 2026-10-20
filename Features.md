Below what has been implemented and validated with rathena packetver 20120307

# Account
- Login
- Login modes of rust-ro servers: the login screen asks the selected server how it takes logins (`CA_SERVER_INFO`) and shows a name and password, a name and a sex choice for an address that logs in without an account, or a Register screen (name, password twice, sex) when the server creates accounts (`CA_CREATE_ACCOUNT`); a created account logs in at once
- Server list selection
- Character select (renders each slot's sprite)
- Character creation
- Character deletion
- Join / enter game
- Change character / return to select
- Window layout persistence (restored once per login, saved on quit/logout/change-char)

# Environment
## Rendering
- Map ground (batched by texture, lightmap atlas)
- Water
- Day / night cycle (light tint fade, world lighting)
- Lightmap
- Weather: fog, cloud, firework, snow, leaves
- Ambient RSW effects: e.g. torch, smoke, bubble, fog
- Animated model: windmill, mill
- Distortion (effect hallucination)
- Mob / entity sprites
- GR2 3D models (emperium, guardians): pure-Rust Oodle0 + Bink texture decoders, skeletal animation
- Minimap (party members with names on hover, same-map guild members, server marks: quest + guide directions)
- World map (Ctrl+`): current-map + player position, party markers with names, per-map minimap inset
- Floor item shadow

# Effects
**All effect IDs (from 1 to 1050) are handled** through two generic players plus custom-coded families.

## Generic players
- **SPR**: sprite-sheet / ACT animated particle effects
- **STR**: animated multi-layer texture effects (`.str`), including angle, additive/alpha blend, repeat/loop

## Custom effect families
### Casting & preparation animations
- Begin-spell rings (`begin_spell`, `begin_spell_6`, `begin_spell_8`)
- Cast circles / casting rings (`cast_circle`, `casting_ring`)
- Saint casting, color casting, flower cast
- Couple / heart casting (wedding & linked casts)
- Asura preparation, ready-portal preparation

### Melee & physical skill hits
- Bash, Bash 3D variants, Magnum Break, Bowling Bash
- Pierce, Spear Boomerang, Grimtooth
- Sonic Blow hit, Triple Attack, Chookgi (finger offensive)
- Storm Kick, Sphere Wind, Jump Kick / Jump Body
- Asura Strike body, Guillotine Fist (gumgang / gumgang2)
- Sonic / Soul strike melee (sma), generic slash
- Cart Revolution, Cart Termination, throw-item (Assassin)
- Curse attack, pressure

### Single-target magic & bolts
- Elemental bolts (fire / cold / lightning) via `magic_bolt`
- Soul Strike, Napalm Beat / Napalm Vulcan
- Fireball, Frost Diver, Grimtooth, Water Ball (2 variants)
- Kouenka (fire), electric bolts, cloud projectile
- Soul Breaker

### Area / ground magic
- Storm Gust, Heaven's Drive, Sightrasher
- Thunderstorm, Jupitel Thunder (yupitel / yufitel2)
- Meteor / Volcano, Sand Wind, Earth Spike, Waterfall
- Gravitation, Grand Cross (walls / caps, additive)

### Ground unit / placed skills (`bottom_*`)
- Sanctuary pillar, Magnus Exorcismus, song / dance
- Volcano / Deluge / Whirlwind, Land Protector, Hermode
- Safety Wall & Glass Wall, Ice Wall
- Fire Pillar, Basilica, box / light / vertical / out zones
- Warp portal, big portal, portal wind, warp zone

### Buffs & self auras
- Blessing, Increase AGI (agiup), Haste up
- Overthrust, Endure, Guard, Defender, Providence
- Aura Blade, Enhance, spirit orbit (rg_coin)
- Two-Hand Quicken, Energy Coat / body buff (body tint + afterimage)
- Status-up sparkles, peong / peong-up

### Support / heal
- Heal, Heal-SP, First Aid
- Turn Undead, Resurrection / revive
- Soul Link, line link (party links)

### Body-channel effects
- Body tint, body scale, quake body, multi-body afterimage
- Spined body, square body, 4-way body, land body

### Hit / impact markers
- Generic hit / hit2 / hit5-6, hit line, dark hit
- Cold hit, fire-splash hit, tei hit, spike burst

### Status ailments (visual)
- Poison, Curse, Silence, Stun, Sleep
- Freeze (ice-block overlay + shatter), Stone / Stone-wait
- Blind (fullscreen overlay), Bleeding
- Root, Blade Stop (rooted)

### Level & job auras
- Level-99 aura ring, transcendent sparkles, orbs (3-layer composite)

### Environmental / ambient
- Cloud, wind, sakura (cherry blossom), color paper, rainbow
- Forest light, twilight, dragon smoke, fire ivy
- Bubble drop, footprints, sparkle column

### Warp / teleport
- Warp, teleportation, portals, entry / exit

### Numbers & indicators
- Damage-number effect, colored heal / SP numbers

### Summon / special / misc
- Summon slave, super angel, saint wing, black devil, ghost
- Tarot card, wink, detecting, sight, lock-on, light sphere
- Barrier, dome ring, floor aura, map zone / pillar, call zone
- Party effect, energy drain, acid demonstration (chemical / aciddemon), venom dust
- Firecracker (pokjuk), potion effects (berserk / concentration / pillar), sparkle (banjjakii)

# Combat & gameplay
- Sound effects (SFX)
- Background music (BGM)
- Damage numbers (color by source: player red, skill, miss white; multi-hit)
- Entity animation: movement, attack, death, sit, doridori head turn
- Camera rotation locked indoors
- Server-time synced movement & actions
- Arrow / projectile flight (normal attacks + arrow skills)
- Camera shake (e.g. Meteor / earthquake skills)
- Skill casting, cooldowns, cast bars
- Emotions (emote balloons, /commands)
- Full server-time sync of hits & moves
- Marriage system
- Complex skills: (e.g: autocounter, grafiti)

# Status appearance
- Status-icon bar (EFST icons, clock-wedge timer, tooltips)
- Ailment body tints, animation locks, movement blocks
- EFST-driven buff effects

# Stealth
- Hide / Cloaking / Chase Walk (movement & input gates, hidden-viewer render, picking filter)

# UI windows
## Character & info
- Basic info window
- Status window (stats)
- Equipment window with paperdoll
- Inventory window (3 tabs, drag & drop, floor-drop)
- Skill tree window
- Hotkey / shortcut bar
- Shortcut list window (Alt+M chat-command bindings)
- Level-up notification: after a base or job level-up the Status or Skills button in the Basic Info window glows while points are left, until that window is opened (the restore button glows while the panel is folded); item-pickup notification

## Items & interaction
- Item info window + card picture
- Card insert dialog / card slot selection
- Drop-quantity dialog
- Number input / item-list selection
- Book reader
- Confirmation dialog
- Context menu (right-click)
- Auto-loot: walks to and picks up items within 6 cells while idle (QoL toolbar switch or Graphic Settings, off by default); pauses above 80% carry weight unless that option is cleared
- Rare-item glow: floor items glow blue (rare, drops under 2%), purple (epic, under 0.5%) or gold (legendary, 0.1% or less, cycling colours) by how rarely monsters drop them, read from the server's `/db` API; cards glow until that is known
- Hold Alt to show the names of all items on the floor
- QoL toolbar: small draggable toolbar in the bottom-right corner (foldable to a single button) with the autofarm switch, its settings window, the auto-loot switch and the loot filter window
- Autofarm: the toolbar button (or Ctrl+F, once "Enable the autofarm hotkey" is ticked in the Graphic Settings) attacks a random living, non-boss monster in view and picks up the drops. It stops on a world click, death or a map change
- Autofarm settings window: stop above a carry weight (default 80%) and flee below an HP percentage (default 25%), each optionally returning to the save point with a Butterfly Wing; heals below an HP percentage with healing items or the character's own Heal skill; uses SP items below an SP percentage
- Loot filter window: restricts what auto-loot and autofarm pick up by minimum rarity (uncommon under 10%, rare under 2%, epic under 0.5%, legendary 0.1% or less) and by item type, judged from the server's `/db` entry; items it cannot identify are picked up
- Auto-travel: `/travel <map>` (for instance `/travel geffen`) asks the reference server (`/db/route`, needs `reference_url`) for the route to a town and walks it, through warps and warper NPCs that offer the destination; it stops on a world click, death, or a step that fails, and says why
- Database window: browses the server's `/db` reference with search, paging and back/forward history; an entry's plain fields come first, and every nested part is a collapsible section (closed by default, the item description open), with lists of records shown as tables and an expand/collapse-all button
- Poptip / tooltips

## Social
- Chat box / chat window
- Chat room: creation, board, member window
- Party & Friends window (combined tabbed)
- Party helper window
- Party board (Party window, Board): the server's party booking ads in a table (leader, level, place, wanted jobs, age), searched by place or by job (the server takes one or the other) and optionally near your level, ten to a page. Ads going up, changing and coming down show live. Double-click an ad or use Whisper to message its leader. "My ad" posts, changes or removes your own ad: a place and up to six wanted jobs; moving it to another place reposts it. Place names are the client's own list, sent as their index
- Guild window (context menu, invite, members with head sprites, emblem, alliance, antagonist, announcement, expel dialog with reason)
- Emblem picker window
- Emotion selector window
- Trade window (player exchange)
- Warp list window
- Forum window (System menu, Forum): the server's `/forum` threads, a thread with its comments and nested replies, starting a thread and commenting or replying. It signs in by itself as the playing character with the game login. While in game it watches the forum's WebSocket: a reply to your threads or comments shows in chat (one line for a backlog), lands in the window's Replies view with an unread count, and refreshes the thread if it is on show

## NPC & shops
- NPC dialog
- NPC shop
- Vending: setup, my-shop, board, shopping window
- Cart window & cart-select (change cart)

## Companions
- Pet window (info, taming, hatch, feed, performance, talk)
- Homunculus window + skill window
- Mercenary window + skill window
- Companion (Homunculus / Mercenary) AI configuration window (JSON-configurable native AI)

## Storage & mail
- Storage / Kafra warehouse window
- Mailbox window + read-mail window

## Crafting / production
- Make-item window (potion / weapon / arrow / refine)

## System
- Graphic options window (fog / effects / aura toggles, runtime)
- Sound options window
- System / game menu
- Map-missing window
- Shortcut configuration
- Slash command

# Game mechanics
## Items & equipment
- Slot / card insertion into equipment
- Equip / unequip with paperdoll rendering

## Crafting
- Potion creation (pharmacy)
- Weapon forging
- Weapon refining
- Arrow crafting

## Social systems
- Party (invite, members, context menu)
- Friends list
- Guild (invite, members, alliance, antagonist, announcement, exclusion)
- Chat rooms
- Trade between players
- Vending (open shop from cart, browse others)
- Mail (send / read)
- Woe: emblem over head, hide damage

## World & progression
- Quest log (list + detail, NPC markers, minimap dots)
- Marriage / couples (wedding costume, partner name, cut-ins, WE_ skill effects)
- Adoption: Adopt baby, children sprite
- Storage access
- Worldmap

## Companions
- Pet system (taming, roulette, hatching, accessory, performance, talk)
- Homunculus (native AI, feed, skills)
- Mercenary (native AI, skills)
- Falcon companion
- Cart (visual + storage)
