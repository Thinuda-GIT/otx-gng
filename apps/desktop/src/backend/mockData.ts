/** Static seed data for the MockBackend. Realistic, but every value here is FAKE preview data.
 *  The UI shows a "Preview data" chip while the mock backend is active. */
import type {
  Adapter,
  ChangeRecord,
  CommunityEntry,
  DnsServer,
  Game,
  GameProfile,
  ProcessGroup,
  ShaperRule,
  SystemInfo,
  TuneFinding,
  Tweak,
} from "./types";

export const MOCK_SYSTEM: SystemInfo = {
  os: { name: "Windows 11", build: 26100, edition: "Home" },
  cpu: { model: "Intel Core i7-12700H", cores: 14, threads: 20, hybrid: true, pCores: 6, eCores: 8 },
  gpus: [
    { name: "NVIDIA GeForce RTX 3060 Laptop GPU", vendor: "nvidia", vramMB: 6144, integrated: false },
    { name: "Intel Iris Xe Graphics", vendor: "intel", vramMB: 128, integrated: true },
  ],
  ramMB: 15_872,
  disk: { model: "NVMe SSD 1 TB", kind: "nvme" },
  formFactor: "laptop",
  onBattery: false,
  network: { kind: "wifi", name: "Wi-Fi", linkMbps: 866, wifiSignalPct: 72, band: "5 GHz" },
  powerPlan: "balanced",
  startupApps: 11,
  antiCheats: [
    { name: "Riot Vanguard", service: "vgc", running: true },
    { name: "Easy Anti-Cheat", service: "EasyAntiCheat_EOS", running: false },
  ],
};

const defaultProfile = (over: Partial<GameProfile> = {}): GameProfile => ({
  tweakPreset: "high",
  ramCleanOnLaunch: true,
  cpuPriority: "high",
  cpuSets: "auto",
  ioPriority: "normal",
  gpuPreference: "high-performance",
  disableFullscreenOptimizations: false,
  networkProfile: "focus",
  backgroundApps: ["OneDrive.exe", "msedge.exe", "Spotify.exe"],
  ...over,
});

const DAY = 86_400_000;
const now = Date.now();

export const MOCK_GAMES: Game[] = [
  { id: "valorant", name: "Valorant", launcher: "riot", exe: "VALORANT-Win64-Shipping.exe", installPath: "C:\\Riot Games\\VALORANT", sizeGB: 38.2, lastPlayed: now - 0.2 * DAY, tint: "255 70 85", antiCheat: "Riot Vanguard", engine: "unreal", profile: defaultProfile() },
  { id: "fortnite", name: "Fortnite", launcher: "epic", exe: "FortniteClient-Win64-Shipping.exe", installPath: "C:\\Program Files\\Epic Games\\Fortnite", sizeGB: 92.6, lastPlayed: now - 1.5 * DAY, tint: "140 108 255", antiCheat: "Easy Anti-Cheat", engine: "unreal", profile: defaultProfile({ disableFullscreenOptimizations: true }) },
  { id: "cod", name: "Call of Duty", launcher: "battlenet", exe: "cod.exe", installPath: "D:\\Games\\Call of Duty", sizeGB: 148.4, lastPlayed: now - 4 * DAY, tint: "170 210 90", antiCheat: "Ricochet", engine: "other", profile: defaultProfile() },
  { id: "cs2", name: "Counter-Strike 2", launcher: "steam", exe: "cs2.exe", installPath: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Counter-Strike Global Offensive", sizeGB: 33.1, lastPlayed: now - 2 * DAY, tint: "255 176 32", antiCheat: "VAC", engine: "source", profile: defaultProfile() },
  { id: "apex", name: "Apex Legends", launcher: "steam", exe: "r5apex.exe", installPath: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Apex Legends", sizeGB: 71.0, lastPlayed: null, tint: "255 90 60", antiCheat: "Easy Anti-Cheat", engine: "source", profile: defaultProfile() },
  { id: "lol", name: "League of Legends", launcher: "riot", exe: "League of Legends.exe", installPath: "C:\\Riot Games\\League of Legends", sizeGB: 21.4, lastPlayed: now - 7 * DAY, tint: "45 226 255", antiCheat: "Riot Vanguard", engine: "other", profile: defaultProfile({ cpuPriority: "above" }) },
  { id: "gtav", name: "Grand Theft Auto V", launcher: "steam", exe: "GTA5.exe", installPath: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Grand Theft Auto V", sizeGB: 105.3, lastPlayed: now - 12 * DAY, tint: "90 200 120", antiCheat: "BattlEye", engine: "other", profile: defaultProfile() },
  { id: "minecraft", name: "Minecraft", launcher: "xbox", exe: "javaw.exe", installPath: "C:\\XboxGames\\Minecraft Launcher", sizeGB: 1.2, lastPlayed: now - 3 * DAY, tint: "110 200 90", antiCheat: null, engine: "other", profile: defaultProfile({ networkProfile: "none" }) },
  { id: "roblox", name: "Roblox", launcher: "manual", exe: "RobloxPlayerBeta.exe", installPath: "C:\\Users\\Player\\AppData\\Local\\Roblox", sizeGB: 0.9, lastPlayed: now - 5 * DAY, tint: "235 235 245", antiCheat: "Hyperion", engine: "other", profile: defaultProfile({ cpuPriority: "above" }) },
];

type Proc = Omit<ProcessGroup, "cpu" | "gpu" | "diskReadKBs" | "diskWriteKBs" | "netDownKbps" | "netUpKbps" | "hog"> & {
  base: { cpu: number; gpu: number; disk: number; down: number; up: number };
};

const p = (
  id: string,
  name: string,
  exe: string,
  category: ProcessGroup["category"],
  base: Proc["base"],
  mem: [number, number],
  meta: Partial<Proc> = {},
): Proc => ({
  id,
  name,
  exe,
  category,
  pids: meta.pids ?? [1000 + Math.floor(Math.random() * 30000)],
  gpuEngine: base.gpu > 0 ? "3D" : "-",
  wsMB: mem[0],
  privateMB: mem[1],
  commitMB: Math.round(mem[1] * 1.1),
  protected: false,
  publisher: "Unknown",
  signed: true,
  description: "",
  path: "",
  startup: false,
  safeToClose: "caution",
  priority: "normal",
  eco: false,
  base,
  ...meta,
});

export const MOCK_PROCESSES: Proc[] = [
  p("discord", "Discord", "Discord.exe", "background", { cpu: 1.8, gpu: 0.6, disk: 20, down: 64, up: 48 }, [612, 420], { pids: [4120, 4188, 4402, 5530, 5612], publisher: "Discord Inc.", description: "Voice and text chat. Keeps full network in Focus mode so voice chat keeps working.", path: "C:\\Users\\Player\\AppData\\Local\\Discord\\app-1.0.9200\\Discord.exe", startup: true, safeToClose: "yes" }),
  p("msedge", "Microsoft Edge", "msedge.exe", "browser", { cpu: 3.4, gpu: 2.1, disk: 60, down: 380, up: 22 }, [1480, 1120], { pids: [7012, 7044, 7090, 7166, 7230, 7301, 7380, 7412], publisher: "Microsoft Corporation", description: "Web browser. Open tabs, especially video, use CPU, GPU and memory.", path: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe", startup: true, safeToClose: "yes" }),
  p("chrome", "Google Chrome", "chrome.exe", "browser", { cpu: 4.9, gpu: 3.4, disk: 90, down: 920, up: 40 }, [2210, 1840], { pids: [8800, 8812, 8840, 8901, 8977, 9010, 9050, 9120, 9180, 9240], publisher: "Google LLC", description: "Web browser with many child processes (one per tab or extension).", path: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", safeToClose: "yes" }),
  p("steam", "Steam", "steam.exe", "launcher", { cpu: 0.9, gpu: 0.2, disk: 2400, down: 18500, up: 120 }, [380, 260], { pids: [3010, 3090, 3104], publisher: "Valve Corp.", description: "Game launcher. It is downloading an update right now, which uses most of the bandwidth.", path: "C:\\Program Files (x86)\\Steam\\steam.exe", startup: true, safeToClose: "caution" }),
  p("riot", "Riot Client", "RiotClientServices.exe", "launcher", { cpu: 0.6, gpu: 0.1, disk: 8, down: 12, up: 6 }, [240, 180], { publisher: "Riot Games, Inc.", description: "Riot launcher. Needed to start Valorant and League of Legends.", path: "C:\\Riot Games\\Riot Client\\RiotClientServices.exe", safeToClose: "no" }),
  p("onedrive", "OneDrive", "OneDrive.exe", "background", { cpu: 0.7, gpu: 0, disk: 640, down: 40, up: 2600 }, [160, 110], { publisher: "Microsoft Corporation", description: "Cloud file sync. Uploads can raise ping through bufferbloat; it can be paused during games.", path: "C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe", startup: true, safeToClose: "yes" }),
  p("spotify", "Spotify", "Spotify.exe", "background", { cpu: 1.1, gpu: 0.3, disk: 30, down: 320, up: 4 }, [420, 300], { pids: [6110, 6150, 6188], publisher: "Spotify AB", description: "Music streaming.", path: "C:\\Users\\Player\\AppData\\Roaming\\Spotify\\Spotify.exe", startup: true, safeToClose: "yes" }),
  p("searchindexer", "Windows Search", "SearchIndexer.exe", "system", { cpu: 0.8, gpu: 0, disk: 900, down: 0, up: 0 }, [92, 60], { publisher: "Microsoft Windows", description: "Indexes files for Start menu search. Can cause disk activity after large file changes.", path: "C:\\Windows\\System32\\SearchIndexer.exe", safeToClose: "no" }),
  p("msmpeng", "Microsoft Defender Antivirus", "MsMpEng.exe", "system", { cpu: 1.5, gpu: 0, disk: 400, down: 2, up: 1 }, [260, 220], { protected: true, publisher: "Microsoft Windows", description: "Antivirus engine. OTX.GNG never touches it.", path: "C:\\ProgramData\\Microsoft\\Windows Defender\\Platform\\MsMpEng.exe", safeToClose: "no" }),
  p("dwm", "Desktop Window Manager", "dwm.exe", "system", { cpu: 1.2, gpu: 4.5, disk: 0, down: 0, up: 0 }, [140, 90], { protected: true, publisher: "Microsoft Windows", description: "Draws every window on screen. Protected.", path: "C:\\Windows\\System32\\dwm.exe", safeToClose: "no" }),
  p("audiodg", "Windows Audio Device Graph", "audiodg.exe", "system", { cpu: 0.4, gpu: 0, disk: 0, down: 0, up: 0 }, [28, 20], { protected: true, publisher: "Microsoft Windows", description: "Audio processing. Protected so game and voice audio never break.", path: "C:\\Windows\\System32\\audiodg.exe", safeToClose: "no" }),
  p("csrss", "Client Server Runtime", "csrss.exe", "system", { cpu: 0.1, gpu: 0, disk: 0, down: 0, up: 0 }, [6, 2], { protected: true, publisher: "Microsoft Windows", description: "Core Windows process. Protected.", path: "C:\\Windows\\System32\\csrss.exe", safeToClose: "no" }),
  p("lsass", "Local Security Authority", "lsass.exe", "system", { cpu: 0.1, gpu: 0, disk: 0, down: 0, up: 0 }, [22, 10], { protected: true, publisher: "Microsoft Windows", description: "Handles sign-in and security. Protected.", path: "C:\\Windows\\System32\\lsass.exe", safeToClose: "no" }),
  p("vgc", "Riot Vanguard (user service)", "vgc.exe", "system", { cpu: 0.2, gpu: 0, disk: 0, down: 4, up: 2 }, [30, 18], { protected: true, publisher: "Riot Games, Inc.", description: "Anti-cheat service. OTX.GNG never touches anti-cheat processes.", path: "C:\\Program Files\\Riot Vanguard\\vgc.exe", safeToClose: "no" }),
  p("nvcontainer", "NVIDIA Container", "NVDisplay.Container.exe", "system", { cpu: 0.3, gpu: 0, disk: 0, down: 0, up: 0 }, [60, 40], { protected: true, publisher: "NVIDIA Corporation", description: "GPU driver service. Protected.", path: "C:\\Windows\\System32\\DriverStore\\FileRepository\\nvdisplay.container.exe", safeToClose: "no" }),
  p("svchost-wu", "Windows Update (svchost)", "svchost.exe", "system", { cpu: 0.9, gpu: 0, disk: 1200, down: 2400, up: 40 }, [110, 80], { protected: true, publisher: "Microsoft Windows", description: "Hosts Windows Update and Delivery Optimization. OTX.GNG can pause updates for a session but never blocks svchost.", path: "C:\\Windows\\System32\\svchost.exe", safeToClose: "no" }),
  p("explorer", "Windows Explorer", "explorer.exe", "system", { cpu: 0.6, gpu: 0.4, disk: 10, down: 0, up: 0 }, [180, 120], { publisher: "Microsoft Windows", description: "Taskbar, Start and File Explorer.", path: "C:\\Windows\\explorer.exe", safeToClose: "no" }),
  p("teams", "Microsoft Teams", "ms-teams.exe", "background", { cpu: 1.3, gpu: 0.8, disk: 20, down: 30, up: 12 }, [520, 400], { pids: [9900, 9932, 9950], publisher: "Microsoft Corporation", description: "Work chat. Can be closed while you play.", path: "C:\\Program Files\\WindowsApps\\MSTeams\\ms-teams.exe", startup: true, safeToClose: "yes" }),
  p("updater", "Unknown updater", "AppUpdateHelper.exe", "unknown", { cpu: 2.6, gpu: 0, disk: 300, down: 600, up: 10 }, [70, 50], { publisher: "Unknown publisher", signed: false, description: "Not in the offline knowledge file. Unsigned. Check the path before closing.", path: "C:\\Users\\Player\\AppData\\Local\\Temp\\AppUpdateHelper.exe", safeToClose: "caution" }),
  p("otx", "OTX.GNG", "OTX.GNG.exe", "background", { cpu: 0.4, gpu: 0.2, disk: 2, down: 0, up: 0 }, [96, 64], { pids: [2210, 2240, 2251], publisher: "OTX.GNG", description: "This app. Its UI pauses while a game is in the foreground.", path: "C:\\Program Files\\OTX.GNG\\OTX.GNG.exe", safeToClose: "yes" }),
];

export const MOCK_GAME_PROCESS = (g: Game): Proc =>
  p(`game-${g.id}`, g.name, g.exe, "game", { cpu: 38, gpu: 82, disk: 120, down: 95, up: 60 }, [6200, 5400], {
    publisher: "Game publisher",
    description: "The running game. OTX.GNG only changes its priority through documented Windows APIs; it never reads or writes game memory.",
    path: `${g.installPath}\\${g.exe}`,
    safeToClose: "caution",
    priority: "high",
  });

export const MOCK_TWEAKS: Omit<Tweak, "enabled" | "notRecommendedIf">[] = [
  { id: "ue-config", group: "game", icon: "FileCog", title: "Unreal Engine config tuner", description: "Applies performance-neutral settings to UE4/UE5 games you have launched at least once. Shows a diff and backs up first.", risk: "moderate", evidence: "experimental", beta: true, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "gpu-pref", group: "game", icon: "Cpu", title: "High-performance GPU for games", description: "Sets the Windows per-app graphics preference so detected games use the discrete GPU.", risk: "safe", evidence: "documented", beta: true, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "hags", group: "input", icon: "MousePointerClick", title: "Hardware-accelerated GPU scheduling", description: "Lets the GPU manage its own memory queue. Can lower latency slightly with recent drivers.", risk: "moderate", evidence: "community", beta: false, requiresReboot: true, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "key-repeat", group: "input", icon: "Keyboard", title: "Faster key repeat", description: "Shortest repeat delay and fastest repeat rate for held keys. Changes how typing feels, not FPS.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "perf-counters", group: "system", icon: "Gauge", title: "Turn off third-party performance counters", description: "Disables extensible counter providers from other apps. Windows and OTX.GNG counters stay on. Gains are rarely measurable.", risk: "advanced", evidence: "experimental", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true, conflictNote: "Built-in CPU, memory, disk and GPU counters are excluded so Task Manager and the Usage Monitor keep working. If GPU counters ever stop responding, the GPU columns turn off and say why." },
  { id: "fg-boost", group: "system", icon: "Rocket", title: "Foreground priority boost", description: "Gives the app in focus (your game) longer CPU time slices than background apps.", risk: "safe", evidence: "community", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "bg-priority", group: "system", icon: "ArrowDownWideNarrow", title: "Lower priority for background apps", description: "During game sessions, background apps run at Below Normal priority in efficiency mode. Undone when the game closes.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "nic-power", group: "network", icon: "Wifi", title: "Network adapter power saving off", description: "Stops Windows from putting the network adapter to sleep, which can cause brief lag spikes.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "game-dvr", group: "game", icon: "Video", title: "Background game capture off", description: "Turns off Game Bar background recording, which encodes video while you play.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "core-unpark", group: "power", icon: "Layers", title: "Keep all CPU cores awake", description: "Stops the power plan from parking cores, so they respond instantly. Uses more power.", risk: "moderate", evidence: "community", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "ntfs-lastaccess", group: "system", icon: "HardDrive", title: "Skip file last-access updates", description: "Stops NTFS writing a timestamp every time a file is read. Many systems already have this off.", risk: "safe", evidence: "documented", beta: false, requiresReboot: true, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "sysmain", group: "system", icon: "MemoryStick", title: "SysMain (prefetch service) off", description: "Stops Windows preloading apps into memory. Can reduce background disk use on fast SSDs.", risk: "moderate", evidence: "community", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "search-index", group: "system", icon: "Search", title: "Search indexing off", description: "Stops background file indexing. Start menu and file search become slower.", risk: "moderate", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "ultimate-plan", group: "power", icon: "Zap", title: "Ultimate Performance power plan", description: "Adds and activates the Ultimate Performance plan, or High performance where it is unavailable.", risk: "moderate", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "game-mode", group: "game", icon: "Gamepad2", title: "Windows Game Mode", description: "Windows' own game prioritisation. On by default in Windows; keep it on unless a specific game misbehaves.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "mouse-accel", group: "input", icon: "Mouse", title: "Mouse acceleration off", description: "Turns off Enhance pointer precision, so the same hand movement always moves the cursor the same distance.", risk: "safe", evidence: "documented", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: true, supported: true },
  { id: "net-throttle", group: "network", icon: "Gauge", title: "Network throttling index off", description: "Stops Windows limiting non-multimedia network processing while media plays.", risk: "moderate", evidence: "community", beta: false, requiresReboot: true, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "nagle", group: "network", icon: "Network", title: "Faster TCP acknowledgements", description: "Turns off Nagle batching and delayed ACKs on the active adapter. Only helps TCP games; most shooters use UDP.", risk: "moderate", evidence: "experimental", beta: false, requiresReboot: true, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "mmcss-games", group: "game", icon: "SlidersHorizontal", title: "Multimedia scheduler Games profile", description: "Raises the Games profile in the multimedia scheduler. Only affects games that register with it, and few do.", risk: "moderate", evidence: "experimental", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: true },
  { id: "timer-res", group: "system", icon: "Timer", title: "Timer resolution", description: "Requests a finer system timer. Windows 10 2004+ and Windows 11 changed how this works.", risk: "advanced", evidence: "experimental", beta: false, requiresReboot: false, appliesTo: ["win10", "win11"], inHighPerformance: false, supported: false, unsupportedReason: "Hidden until a Benchmark Lab run on this PC shows a measurable gain. On Windows 11 the request has no effect on other apps unless an extra system setting is changed." },
];

/** Rules are evaluated against MOCK_SYSTEM by the mock. */
export const MOCK_NOT_RECOMMENDED: Record<string, { label: string; when: (s: SystemInfo) => boolean }[]> = {
  "ue-config": [
    { label: "The game's rules forbid editing its config files", when: () => false },
    { label: "You have not launched the game since installing it", when: () => false },
  ],
  "gpu-pref": [{ label: "This PC has no discrete GPU", when: (s) => !s.gpus.some((g) => !g.integrated) }],
  hags: [{ label: "Your GPU driver is older than 2021 or a game shows stutter after enabling it", when: () => false }],
  "perf-counters": [{ label: "You use monitoring tools from other vendors", when: () => false }],
  "nic-power": [{ label: "Laptop running on battery", when: (s) => s.formFactor === "laptop" && s.onBattery }],
  "game-dvr": [{ label: "You record clips with Game Bar", when: () => false }],
  "core-unpark": [{ label: "Laptop (more heat and battery use)", when: (s) => s.formFactor === "laptop" }],
  sysmain: [{ label: "Windows is installed on a hard disk (HDD)", when: (s) => s.disk.kind === "hdd" }],
  "search-index": [{ label: "You use Start menu, Outlook or file search often", when: () => false }],
  "ultimate-plan": [{ label: "Laptop (more heat and battery use)", when: (s) => s.formFactor === "laptop" }],
  nagle: [{ label: "Your games use UDP (most shooters)", when: () => true }],
  "net-throttle": [{ label: "You stream or record while playing", when: () => false }],
};

/** What each tweak changes. Shown in dry-run previews. */
export const MOCK_TWEAK_CHANGES: Record<string, string[]> = {
  "ue-config": ["%LOCALAPPDATA%\\<Game>\\Saved\\Config\\Windows\\Engine.ini: backed up, then diff shown before writing"],
  "gpu-pref": ["HKCU\\Software\\Microsoft\\DirectX\\UserGpuPreferences\\<game exe>: GpuPreference=2;"],
  hags: ["HKLM\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers\\HwSchMode: 1 → 2 (reboot)"],
  "key-repeat": ["SPI_SETKEYBOARDDELAY: 1 → 0", "SPI_SETKEYBOARDSPEED: 31 (fastest)"],
  "perf-counters": ["Perflib: third-party extensible providers → Disable Performance Counters = 1 (PerfOS, PerfProc, PerfDisk, GPU excluded)"],
  "fg-boost": ["HKLM\\SYSTEM\\CurrentControlSet\\Control\\PriorityControl\\Win32PrioritySeparation: 0x2 → 0x26"],
  "bg-priority": ["Game Mode session list: background apps → Below Normal + EcoQoS (temporary)"],
  "nic-power": ["Wi-Fi adapter: Allow the computer to turn off this device to save power → Off"],
  "game-dvr": ["HKCU\\System\\GameConfigStore\\GameDVR_Enabled: 1 → 0", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR\\AppCaptureEnabled: 1 → 0"],
  "core-unpark": ["Active power plan → Processor performance core parking min cores (AC): 10% → 100%"],
  "ntfs-lastaccess": ["fsutil behavior disablelastaccess: 2 (system managed) → 1 (disabled)"],
  sysmain: ["Service SysMain: Automatic → Disabled, stopped"],
  "search-index": ["Service WSearch: Automatic (Delayed Start) → Disabled, stopped"],
  "ultimate-plan": ["powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61, then activate"],
  "game-mode": ["HKCU\\Software\\Microsoft\\GameBar\\AutoGameModeEnabled: 1"],
  "mouse-accel": ["SPI_SETMOUSE: MouseSpeed 1 → 0, thresholds 6/10 → 0/0"],
  "net-throttle": ["HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\NetworkThrottlingIndex: 10 → 0xFFFFFFFF"],
  nagle: ["Tcpip\\Parameters\\Interfaces\\{active adapter}\\TcpAckFrequency = 1, TCPNoDelay = 1"],
  "mmcss-games": ["SystemProfile\\Tasks\\Games: GPU Priority 8, Priority 6, Scheduling Category High"],
  "timer-res": [],
};

/** Windows defaults (the "Default" preset). */
export const WINDOWS_DEFAULT_ON = new Set(["game-mode"]);

export const MOCK_DNS: DnsServer[] = [
  { id: "isp", name: "Your ISP (current)", v4: ["192.168.1.1"], v6: [], filtering: false, isp: true, custom: false },
  { id: "cloudflare", name: "Cloudflare", v4: ["1.1.1.1", "1.0.0.1"], v6: ["2606:4700:4700::1111", "2606:4700:4700::1001"], filtering: false, isp: false, custom: false },
  { id: "google", name: "Google Public DNS", v4: ["8.8.8.8", "8.8.4.4"], v6: ["2001:4860:4860::8888", "2001:4860:4860::8844"], filtering: false, isp: false, custom: false },
  { id: "quad9", name: "Quad9", v4: ["9.9.9.9", "149.112.112.112"], v6: ["2620:fe::fe", "2620:fe::9"], filtering: false, isp: false, custom: false },
  { id: "opendns", name: "OpenDNS", v4: ["208.67.222.222", "208.67.220.220"], v6: ["2620:119:35::35", "2620:119:53::53"], filtering: false, isp: false, custom: false },
  { id: "adguard", name: "AdGuard DNS", v4: ["94.140.14.14", "94.140.15.15"], v6: ["2a10:50c0::ad1:ff", "2a10:50c0::ad2:ff"], filtering: true, isp: false, custom: false },
  { id: "verisign", name: "Verisign", v4: ["64.6.64.6", "64.6.65.6"], v6: [], filtering: false, isp: false, custom: false },
];

/** Typical medians (ms) for the mock benchmark. Verisign is shown failing to illustrate how an
 *  unreachable resolver is reported (service availability is UNVERIFIED). */
export const MOCK_DNS_LATENCY: Record<string, { ms: number; fail: number }> = {
  isp: { ms: 24, fail: 0 },
  cloudflare: { ms: 11, fail: 0 },
  google: { ms: 17, fail: 0 },
  quad9: { ms: 21, fail: 0 },
  opendns: { ms: 26, fail: 0 },
  adguard: { ms: 33, fail: 1 },
  verisign: { ms: 0, fail: 10 },
};

export const MOCK_CHANGES: ChangeRecord[] = [
  { id: "c1", at: now - 3_600_000 * 26, title: "Network adapter power saving off", detail: "Wi-Fi adapter power management disabled", kind: "tweak", temporary: false },
  { id: "c2", at: now - 3_600_000 * 26, title: "Background game capture off", detail: "GameDVR_Enabled 1 → 0", kind: "tweak", temporary: false },
  { id: "c3", at: now - 3_600_000 * 3, title: "DNS set to Cloudflare", detail: "Wi-Fi IPv4 1.1.1.1, 1.0.0.1 (previous: automatic)", kind: "dns", temporary: false },
];

export const MOCK_RULES: ShaperRule[] = [
  { id: "r-discord", app: "Discord.exe", action: "allow", priority: "high", onlyWhileGaming: false, essential: true },
  { id: "r-steam", app: "steam.exe", action: "limit", upKbps: 500, downKbps: 5000, priority: "low", onlyWhileGaming: true, essential: false },
  { id: "r-onedrive", app: "OneDrive.exe", action: "block", priority: "low", onlyWhileGaming: true, essential: false },
  { id: "r-chrome", app: "chrome.exe", action: "limit", upKbps: 1000, priority: "normal", onlyWhileGaming: true, essential: false },
];

export const MOCK_ADAPTERS: Adapter[] = [
  { id: "wifi", name: "Wi-Fi (Intel Wi-Fi 6 AX201)", kind: "wifi", connected: true, pingMs: 31, lossPct: 0.4, metric: 35, preferred: false },
  { id: "eth", name: "Ethernet (Realtek PCIe 2.5GbE)", kind: "ethernet", connected: false, pingMs: null, lossPct: null, metric: 25, preferred: true },
  { id: "tether", name: "Phone tether (Remote NDIS)", kind: "tether", connected: false, pingMs: null, lossPct: null, metric: 50, preferred: false },
];

export const MOCK_TUNE: TuneFinding[] = [
  { id: "link", title: "You are on Wi-Fi (5 GHz, 72% signal)", detail: "Wi-Fi adds jitter compared with a cable. If you can, use Ethernet for ranked games.", severity: "info", evidence: "documented" },
  { id: "scan", title: "Periodic Wi-Fi scan spikes detected", detail: "Ping rose by about 40-80 ms roughly every 60 s, a pattern typical of background Wi-Fi scanning. Closing apps that request location or Wi-Fi lists often helps.", severity: "warn", evidence: "community" },
  { id: "bloat", title: "Bufferbloat under upload: grade C", detail: "Latency rose from 31 ms idle to 142 ms while uploading. Capping non-game upload during games fixes most of this.", severity: "warn", evidence: "documented", fixLabel: "Cap background upload" },
  { id: "power", title: "Adapter power saving is on", detail: "The adapter may sleep between packets on battery profiles.", severity: "warn", evidence: "documented", fixLabel: "Turn off power saving" },
  { id: "mtu", title: "MTU 1500 works end to end", detail: "Path MTU discovery found no fragmentation to the test target.", severity: "ok", evidence: "documented" },
  { id: "dns", title: "DNS is fast (11 ms)", detail: "DNS only affects lookups when a connection starts, not in-game ping.", severity: "ok", evidence: "documented" },
  { id: "nagle", title: "TCP acknowledgement tweaks", detail: "Your detected games use UDP, so Nagle/ACK changes are not expected to help.", severity: "info", evidence: "experimental" },
];

export const MOCK_COMMUNITY: CommunityEntry[] = [
  { id: "dns-list-main", kind: "dns", name: "OTX community DNS list", address: "github.com/otx-gng/community-data (dns.json)", region: "Global", pingMs: null, favorite: true, signatureOk: true, source: "Signed JSON (example)" },
  { id: "relay-fra", kind: "relay", name: "Example relay: Frankfurt", address: "relay-fra.example.org:51820", region: "EU West", pingMs: null, favorite: false, signatureOk: true, source: "Community list (example)" },
  { id: "relay-sgp", kind: "relay", name: "Example relay: Singapore", address: "relay-sgp.example.org:51820", region: "Asia", pingMs: null, favorite: false, signatureOk: true, source: "Community list (example)" },
  { id: "relay-bad", kind: "relay", name: "Unverified relay list", address: "raw.example.net/relays.json", region: "Unknown", pingMs: null, favorite: false, signatureOk: false, source: "Signature check failed: rejected" },
];
