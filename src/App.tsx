import { FormEvent, ReactNode, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Archive,
  ArrowClockwise,
  CaretDown,
  CaretUp,
  CheckCircle,
  ClockCounterClockwise,
  Database,
  FloppyDisk,
  FolderSimple,
  Globe,
  Info,
  Lightning,
  LockKey,
  Moon,
  Plus,
  RocketLaunch,
  Sparkle,
  Sun,
  TerminalWindow,
  Trash,
  XCircle,
} from "@phosphor-icons/react";
import { AmbientHalo } from "./components/AmbientHalo";
import "./App.css";

type CodexProfile = {
  id: string;
  name: string;
  providerName: string;
  baseUrl: string;
  model: string;
  requiresOpenaiAuth: boolean;
  keepChatgptLogin: boolean;
  codexHome?: string | null;
  createdAt: string;
  updatedAt: string;
};

type ProfileRecord = CodexProfile & {
  tokenReady: boolean;
};

type HistoryStatus = {
  ok: boolean;
  ready: boolean;
  reason?: string | null;
  currentProvider: string;
  currentModel?: string | null;
  totalThreads: number;
  mismatchedProviderThreads: number;
  mismatchedModelThreads?: number | null;
  sessionFileCount: number;
  sessionIndexCount: number;
};

type CodexRuntimeStatus = {
  authModeChatgpt: boolean;
  openaiApiKeyNull: boolean;
  currentProvider: string;
  currentModel?: string | null;
  providerConfigured: boolean;
  providerName?: string | null;
  providerRequiresOpenaiAuth: boolean;
  providerHasBearerToken: boolean;
  readyForRemote: boolean;
};

type DiagnosticsReport = {
  appVersion: string;
  platform: string;
  codexHome: string;
  configPath: string;
  authPath: string;
  databasePath: string;
  sessionsPath: string;
  sessionIndexPath: string;
  globalStatePath: string;
  configExists: boolean;
  authExists: boolean;
  databaseExists: boolean;
  sessionsExists: boolean;
  sessionIndexExists: boolean;
  globalStateExists: boolean;
  backupCount: number;
  profileCount: number;
  customRootCount: number;
  codexInstallFound: boolean;
  codexInstallCandidates: string[];
  codexProcessCount: number;
  runtimeStatus?: CodexRuntimeStatus | null;
  runtimeError?: string | null;
  historyStatus?: HistoryStatus | null;
  historyError?: string | null;
};

type BackupRecord = {
  id: string;
  label: string;
  createdAt: string;
  path: string;
};

type CustomHistoryRoot = {
  id: string;
  label: string;
  path: string;
  createdAt: string;
  updatedAt: string;
};

type ActivationResult = {
  ok: boolean;
  profileId: string;
  backupId: string;
  codexOpened: boolean;
  runtimeStatus?: CodexRuntimeStatus | null;
  history?: {
    updatedDatabaseRows: number;
    updatedSessionFiles: number;
    updatedSessionIndex: boolean;
    updatedGlobalState: boolean;
  } | null;
};

type ClearApiModeResult = {
  ok: boolean;
  backupId: string;
  codexOpened: boolean;
  runtimeStatus: CodexRuntimeStatus;
};

type VisibleFieldErrors = Partial<Record<"name" | "baseUrl" | "model", string>>;
type Language = "zh" | "en";
type Theme = "dark" | "light";
type AppPage = "setup" | "profiles" | "history" | "guide" | "status";

type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "current"; latestVersion: string; releaseUrl: string }
  | { state: "available"; latestVersion: string; releaseUrl: string }
  | { state: "error"; message: string };

type Preferences = {
  language: Language;
  theme: Theme;
  onboarded: boolean;
};

const PREFERENCES_STORAGE_KEY = "remote-codex-api.preferences.v1";
const APP_VERSION = "0.1.0";
const RELEASE_API_URL = "https://api.github.com/repos/serein431/Remote-Codex-API/releases/latest";
const RELEASES_URL = "https://github.com/serein431/Remote-Codex-API/releases";
const previewCreatedAt = "2026-05-16T09:10:00.000Z";

const previewProfiles: ProfileRecord[] = [
  {
    id: "jmrai",
    name: "JMRAI",
    providerName: "JMRAI",
    baseUrl: "https://jmrai.net/v1",
    model: "gpt-5.5",
    requiresOpenaiAuth: true,
    keepChatgptLogin: true,
    codexHome: null,
    createdAt: previewCreatedAt,
    updatedAt: previewCreatedAt,
    tokenReady: true,
  },
  {
    id: "openrouter-lab",
    name: "OpenRouter Lab",
    providerName: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    model: "openai/gpt-5.5",
    requiresOpenaiAuth: true,
    keepChatgptLogin: true,
    codexHome: null,
    createdAt: previewCreatedAt,
    updatedAt: previewCreatedAt,
    tokenReady: true,
  },
];

const previewStatus: HistoryStatus = {
  ok: true,
  ready: true,
  reason: null,
  currentProvider: "remote-codex-api",
  currentModel: "gpt-5.5",
  totalThreads: 48,
  mismatchedProviderThreads: 3,
  mismatchedModelThreads: null,
  sessionFileCount: 48,
  sessionIndexCount: 48,
};

const previewRuntimeStatus: CodexRuntimeStatus = {
  authModeChatgpt: true,
  openaiApiKeyNull: true,
  currentProvider: "remote-codex-api",
  currentModel: "gpt-5.5",
  providerConfigured: true,
  providerName: "JMRAI",
  providerRequiresOpenaiAuth: true,
  providerHasBearerToken: true,
  readyForRemote: true,
};

const previewDiagnostics: DiagnosticsReport = {
  appVersion: APP_VERSION,
  platform: "preview",
  codexHome: "~/.codex",
  configPath: "~/.codex/config.toml",
  authPath: "~/.codex/auth.json",
  databasePath: "~/.codex/state_5.sqlite",
  sessionsPath: "~/.codex/sessions",
  sessionIndexPath: "~/.codex/session_index.jsonl",
  globalStatePath: "~/.codex/.codex-global-state.json",
  configExists: true,
  authExists: true,
  databaseExists: true,
  sessionsExists: true,
  sessionIndexExists: true,
  globalStateExists: true,
  backupCount: 11,
  profileCount: 2,
  customRootCount: 1,
  codexInstallFound: true,
  codexInstallCandidates: ["/Applications/Codex.app"],
  codexProcessCount: 1,
  runtimeStatus: previewRuntimeStatus,
  historyStatus: previewStatus,
};

const previewBackups: BackupRecord[] = [
  {
    id: "20260516-170846-jmrai",
    label: "JMRAI activation",
    createdAt: "2026-05-16T09:08:46.000Z",
    path: "",
  },
];

const previewHistoryRoots: CustomHistoryRoot[] = [
  {
    id: "wsl-ubuntu-home-projects",
    label: "WSL Ubuntu",
    path: "\\\\wsl.localhost\\Ubuntu\\home\\dgsp\\projects",
    createdAt: previewCreatedAt,
    updatedAt: previewCreatedAt,
  },
];

const copy = {
  zh: {
    appName: "Remote Codex API",
    appSubtitle: "第三方 API 也能直接用 Codex Remote。",
    heroTitle: "第三方 API\n也能用 Codex Remote。",
    heroBody: "先在 Codex 登录 ChatGPT，再启用第三方 API 配置。对话走中转站，Remote、插件和额度查询继续跟随登录态。",
    steps: [
      "切到第三方 API。",
      "快速对齐历史。",
      "Remote 继续可用。",
    ],
    topTech: "系统在线 // 远端接入",
    sideTech: ["配置", "=", "REMOTE.API", "-V1"],
    languageShort: "中",
    languageLabel: "界面语言",
    themeLabel: "外观主题",
    darkTheme: "暗色",
    lightTheme: "亮色",
    chooseLanguage: "首次使用",
    chooseLanguageBody: "先选一个默认语言。稍后可以在右上角继续切换。",
    chooseTheme: "默认外观",
    chooseThemeBody: "先选一个你更顺手的默认外观，稍后也可以随时切换。",
    continue: "开始使用",
    chinese: "简体中文",
    english: "English",
    chineseHint: "更适合日常配置和排查。",
    englishHint: "Better for mixed-language workflow.",
    darkHint: "更沉浸，适合长时间使用。",
    lightHint: "更清爽，信息看起来更轻。",
    profilesTitle: "已有配置",
    profilesEmpty: "还没有配置。点右侧面板开始创建。",
    profilesSummary: "配置管理已移到独立页面，名称、地址和模型会完整显示。",
    manageProfiles: "管理配置",
    newProfile: "新建配置",
    pageSetup: "接入",
    pageProfiles: "配置",
    pageHistory: "历史",
    pageGuide: "教程",
    pageStatus: "状态",
    quickPanelBadge: "REMOTE.SETUP",
    quickPanelTitle: "快速配置",
    quickPanelBody: "只填四项：名称、API 地址、模型、Key。点保存并启用后，软件会写入 Codex 配置并重启 Codex。",
    fieldName: "配置名称",
    fieldNameHelper: "也会作为 Codex 里显示的 API 名称，比如 JMRAI 或 OpenRouter。",
    fieldUrl: "API 地址",
    fieldUrlHelper: "填写兼容 OpenAI 的 `/v1` 地址。",
    fieldModel: "模型",
    fieldModelHelper: "例如 `gpt-5.5`、`gpt-4.1`。",
    fieldToken: "API Key",
    fieldTokenHelperNew: "首次启用时必须填写，只会保存到系统钥匙串。",
    fieldTokenHelperExisting: "留空即可继续使用已保存的 Key。",
    guideBadge: "HOW.IT.WORKS",
    guideTitle: "正确使用顺序",
    guideIntro: "这个流程的关键是先保留 ChatGPT 登录，再把模型请求切到第三方 API。",
    guideSteps: [
      "先打开 Codex，完成 ChatGPT 登录，确认 Remote 或插件入口已经跟账号绑定。",
      "回到 Remote Codex API，填写第三方 API 配置和 Key。",
      "点击保存并启用。软件会备份当前文件，再写入 auth.json 和 config.toml。",
      "Codex 重启后，对话走第三方中转，Remote、插件和额度查询继续使用 ChatGPT 登录态。",
    ],
    writesTitle: "启用后会写入",
    writesAuth: "auth.json 保持 ChatGPT 模式，并把 OPENAI_API_KEY 置空。",
    writesConfig: "config.toml 固定使用 Remote Codex API Provider，并更新 requires_openai_auth 与 bearer token。",
    fieldsTitle: "四个框怎么填",
    fieldGuideRows: [
      ["配置名称", "随便取，建议用供应商名，比如 JMRAI。"],
      ["API 地址", "填供应商给的 OpenAI 兼容 /v1 地址。"],
      ["模型", "填你要在 Codex 里使用的模型名。"],
      ["API Key", "填供应商 Key；只保存进系统钥匙串。"],
    ],
    guideWarning: "如果只点仅保存，不会切换 Codex。必须点保存并启用。",
    statusBadge: "LIVE.CHECK",
    statusTitle: "Codex 配置自检",
    statusIntro: "这里直接读取当前 Codex 文件，不靠猜测。",
    remoteReady: "Remote 解锁配置已就绪",
    remoteNotReady: "还没有达到解锁配置",
    authModeLabel: "auth_mode = chatgpt",
    apiKeyNullLabel: "OPENAI_API_KEY = null",
    providerBlockLabel: "当前 Provider 已配置",
    bearerTokenLabel: "experimental_bearer_token 已写入",
    requiresAuthLabel: "requires_openai_auth = true",
    currentConfigTitle: "当前读取到",
    statusRefreshHint: "启用后如果这里仍未就绪，说明 Codex 文件没有写到位。",
    clearApiTitle: "回到官方登录态",
    clearApiIntro: "会先备份，再移除 Remote Codex API 的 Provider 和明文 token，auth.json 继续保持 ChatGPT 登录。",
    clearApiMode: "清除 API 模式",
    clearApiConfirm: "这会备份当前 Codex 文件，并移除 Remote Codex API 写入的 Provider。继续吗？",
    clearApiNotice: "已清除 API 模式，Codex 已回到 ChatGPT 登录态。",
    clearApiBackupNotice: "已清除 API 模式，备份：",
    diagnosticsTitle: "诊断报告",
    diagnosticsIntro: "复制给 issue 或自己排查用；不会输出 bearer token。",
    diagnosticsCopy: "复制诊断",
    diagnosticsCopied: "诊断报告已复制。",
    diagnosticsInstall: "Codex 安装",
    diagnosticsProcess: "Codex 进程",
    diagnosticsProfiles: "配置数",
    diagnosticsRoots: "自定义目录",
    diagnosticsBackups: "备份数",
    diagnosticsFiles: "关键文件",
    updateTitle: "更新检查",
    updateIntro: "从 GitHub Release 检查新版本。",
    updateCheck: "检查更新",
    updateOpen: "打开 Release",
    updateCurrent: "当前已经是最新版。",
    updateAvailable: "发现新版本",
    updateFailed: "检查更新失败。",
    currentVersion: "当前版本",
    latestVersion: "最新版本",
    compatibilityTitle: "增强能力边界",
    compatibilityIntro: "Remote Codex API 专注登录态、Provider 和历史同步。会话删除、导出、移动、Timeline、脚本注入这类桌面增强，建议和 Codex Mate 搭配使用。",
    keepLogin: "保留 ChatGPT 登录",
    keychainSaved: "Key 存进系统钥匙串",
    launchOptions: "启用时选项",
    syncOnActivate: "启用时同步本地历史",
    restartOnActivate: "启用后重启 Codex",
    advanced: "高级设置",
    advancedOpen: "展开高级设置",
    advancedClose: "收起高级设置",
    fieldCodexHome: "Codex 主目录",
    fieldCodexHomeHelper: "留空时默认使用 `~/.codex`。",
    saveActivate: "保存并启用",
    saveOnly: "仅保存",
    openCodex: "打开 Codex",
    deleteProfile: "删除当前配置",
    localState: "本地状态",
    historyAligned: "历史已对齐",
    historyNeedsSync: "历史需要同步",
    historyChecking: "正在检查",
    historySync: "快速同步历史",
    historyHint: "同步只对齐 Provider，不会改动历史里的模型名；历史很多时也能很快完成。",
    historyPageTitle: "历史管理",
    historyPageIntro: "同步本地历史索引，也可以手动加入 WSL 或自定义工作目录。",
    historyRootTitle: "自定义工作目录",
    historyRootIntro: "这些目录会写入 Codex 的项目根列表，适合 Windows WSL 或自动识别不到的路径。",
    fieldRootLabel: "目录名称",
    fieldRootPath: "目录路径",
    addRoot: "添加目录",
    rootsEmpty: "还没有自定义目录。",
    deleteRoot: "删除目录",
    providerLabel: "Codex Provider",
    apiProviderLabel: "当前 API 配置",
    modelLabel: "当前模型",
    threadsLabel: "线程数",
    mismatchLabel: "Provider 未对齐",
    sessionsLabel: "会话文件",
    indexLabel: "索引条目",
    backupsTitle: "最近备份",
    backupsEmpty: "还没有备份记录。",
    restorePrefix: "恢复备份",
    refreshState: "刷新状态",
    saveNotice: "配置已保存。",
    saveWithTokenNotice: "配置已保存，Key 已写入系统钥匙串。",
    deletedNotice: "配置已删除。",
    newDraftNotice: "已切换到新建配置。",
    openedCodexNotice: "Codex 已打开。",
    syncedNotice: "本地历史已同步。",
    restoredNotice: "备份已恢复。",
    activatedPrefix: "已启用",
    activationVerifiedPrefix: "已写入并通过自检",
    activationNeedsCheck: "已写入，但自检还未就绪。请看状态页。",
    validationName: "请填写配置名称。",
    validationUrl: "请填写有效的 API 地址。",
    validationModel: "请填写模型名称。",
    validationGeneric: "请先补全高亮字段。",
    tokenRequired: "首次启用前请填写 API Key。",
    profileSelected: "已载入配置",
    editProfile: "编辑",
    useProfile: "使用",
    providerTag: "供应商",
    activeProfileTag: "当前配置",
    readyLabel: "就绪",
    missingLabel: "待填写",
    keyReady: "Key 已准备",
    keyMissing: "Key 必填",
    pathLabel: "工作目录",
    helperLabel: "这 4 项就够了",
    profileCountLabel: "配置数量",
    backupAction: "恢复",
    openLanguageSwitcher: "切换语言",
    themeSwitcher: "切换主题",
  },
  en: {
    appName: "Remote Codex API",
    appSubtitle: "Use Codex Remote with third-party APIs.",
    heroTitle: "Third-party APIs\nstill work with Codex Remote.",
    heroBody: "Sign in to ChatGPT in Codex first, then activate a third-party API profile. Model traffic goes through your provider while Remote, plugins, and quota stay tied to the login.",
    steps: [
      "Switch to a third-party API.",
      "Realign history fast.",
      "Keep Remote working.",
    ],
    topTech: "SYS.CORE // REMOTE LINK",
    sideTech: ["PROFILE", "=", "REMOTE.API", "-V1"],
    languageShort: "EN",
    languageLabel: "Interface language",
    themeLabel: "Theme",
    darkTheme: "Dark",
    lightTheme: "Light",
    chooseLanguage: "First launch",
    chooseLanguageBody: "Pick your default interface language first. You can switch again from the top right any time.",
    chooseTheme: "Default appearance",
    chooseThemeBody: "Choose the appearance you want to start with. You can switch it again any time.",
    continue: "Continue",
    chinese: "简体中文",
    english: "English",
    chineseHint: "Best for daily setup and troubleshooting.",
    englishHint: "Best for mixed-language workflow.",
    darkHint: "More immersive for long sessions.",
    lightHint: "Brighter and easier to scan.",
    profilesTitle: "Saved profiles",
    profilesEmpty: "No profiles yet. Start from the setup panel on the right.",
    profilesSummary: "Profiles now live on their own page, with full names, URLs, and models visible.",
    manageProfiles: "Manage profiles",
    newProfile: "New profile",
    pageSetup: "Setup",
    pageProfiles: "Profiles",
    pageHistory: "History",
    pageGuide: "Guide",
    pageStatus: "Status",
    quickPanelBadge: "REMOTE.SETUP",
    quickPanelTitle: "Quick setup",
    quickPanelBody: "Fill only four fields: name, API URL, model, and key. Save & activate writes Codex config and restarts Codex.",
    fieldName: "Profile name",
    fieldNameHelper: "Also used as the API display name inside Codex, like JMRAI or OpenRouter.",
    fieldUrl: "API base URL",
    fieldUrlHelper: "Use an OpenAI-compatible `/v1` endpoint.",
    fieldModel: "Model",
    fieldModelHelper: "For example `gpt-5.5` or `gpt-4.1`.",
    fieldToken: "API key",
    fieldTokenHelperNew: "Required on first activation. It is stored only in the system keychain.",
    fieldTokenHelperExisting: "Leave blank to keep the existing key.",
    guideBadge: "HOW.IT.WORKS",
    guideTitle: "Correct order",
    guideIntro: "The trick is keeping the ChatGPT login while routing model requests to your third-party API.",
    guideSteps: [
      "Open Codex first, sign in with ChatGPT, and confirm Remote or plugins are tied to the account.",
      "Return to Remote Codex API and fill the third-party API profile and key.",
      "Click Save & activate. The app backs up current files, then writes auth.json and config.toml.",
      "After Codex restarts, chats go through your provider while Remote, plugins, and quota use the ChatGPT login.",
    ],
    writesTitle: "What activation writes",
    writesAuth: "auth.json stays in ChatGPT mode and OPENAI_API_KEY is set to null.",
    writesConfig: "config.toml keeps the Remote Codex API provider bucket and updates requires_openai_auth plus the bearer token.",
    fieldsTitle: "What each field means",
    fieldGuideRows: [
      ["Profile name", "Any name you recognize, such as JMRAI."],
      ["API base URL", "The provider's OpenAI-compatible /v1 endpoint."],
      ["Model", "The model name you want Codex to use."],
      ["API key", "Your provider key; it is stored only in the system keychain."],
    ],
    guideWarning: "Save only does not switch Codex. Use Save & activate.",
    statusBadge: "LIVE.CHECK",
    statusTitle: "Codex config check",
    statusIntro: "This reads the live Codex files instead of guessing.",
    remoteReady: "Remote unlock config is ready",
    remoteNotReady: "Unlock config is not ready yet",
    authModeLabel: "auth_mode = chatgpt",
    apiKeyNullLabel: "OPENAI_API_KEY = null",
    providerBlockLabel: "Current provider configured",
    bearerTokenLabel: "experimental_bearer_token written",
    requiresAuthLabel: "requires_openai_auth = true",
    currentConfigTitle: "Currently detected",
    statusRefreshHint: "If this is still not ready after activation, Codex files were not written correctly.",
    clearApiTitle: "Return to official login",
    clearApiIntro: "Creates a backup, removes the Remote Codex API provider and plaintext token, and keeps auth.json in ChatGPT login mode.",
    clearApiMode: "Clear API mode",
    clearApiConfirm: "This backs up current Codex files and removes the provider written by Remote Codex API. Continue?",
    clearApiNotice: "API mode cleared. Codex is back on ChatGPT login.",
    clearApiBackupNotice: "API mode cleared. Backup:",
    diagnosticsTitle: "Diagnostics report",
    diagnosticsIntro: "Copy this for issues or troubleshooting. Bearer tokens are never included.",
    diagnosticsCopy: "Copy diagnostics",
    diagnosticsCopied: "Diagnostics copied.",
    diagnosticsInstall: "Codex install",
    diagnosticsProcess: "Codex process",
    diagnosticsProfiles: "Profiles",
    diagnosticsRoots: "Custom roots",
    diagnosticsBackups: "Backups",
    diagnosticsFiles: "Key files",
    updateTitle: "Update check",
    updateIntro: "Checks GitHub Releases for a newer build.",
    updateCheck: "Check updates",
    updateOpen: "Open Releases",
    updateCurrent: "You are on the latest release.",
    updateAvailable: "New version available",
    updateFailed: "Update check failed.",
    currentVersion: "Current version",
    latestVersion: "Latest version",
    compatibilityTitle: "Enhancement boundary",
    compatibilityIntro: "Remote Codex API focuses on login state, providers, and history sync. Desktop enhancements such as delete, export, move, Timeline, and script injection are better paired with Codex Mate.",
    keepLogin: "Keep ChatGPT login",
    keychainSaved: "Key stored in system keychain",
    launchOptions: "Activation options",
    syncOnActivate: "Sync local history on activation",
    restartOnActivate: "Restart Codex after activation",
    advanced: "Advanced settings",
    advancedOpen: "Show advanced settings",
    advancedClose: "Hide advanced settings",
    fieldCodexHome: "Codex home",
    fieldCodexHomeHelper: "Blank uses `~/.codex`.",
    saveActivate: "Save & activate",
    saveOnly: "Save only",
    openCodex: "Open Codex",
    deleteProfile: "Delete current profile",
    localState: "Local state",
    historyAligned: "History aligned",
    historyNeedsSync: "History needs sync",
    historyChecking: "Checking",
    historySync: "Fast history sync",
    historyHint: "Sync only aligns the provider bucket and leaves historical model names untouched.",
    historyPageTitle: "History management",
    historyPageIntro: "Sync local history indexes and add WSL or custom workspace roots manually.",
    historyRootTitle: "Custom workspace roots",
    historyRootIntro: "These paths are written to Codex project roots for Windows WSL or paths auto-detection misses.",
    fieldRootLabel: "Root label",
    fieldRootPath: "Root path",
    addRoot: "Add root",
    rootsEmpty: "No custom roots yet.",
    deleteRoot: "Delete root",
    providerLabel: "Codex provider",
    apiProviderLabel: "Active API profile",
    modelLabel: "Current model",
    threadsLabel: "Threads",
    mismatchLabel: "Provider mismatched",
    sessionsLabel: "Session files",
    indexLabel: "Index rows",
    backupsTitle: "Recent backups",
    backupsEmpty: "No backups yet.",
    restorePrefix: "Restore backup",
    refreshState: "Refresh state",
    saveNotice: "Profile saved.",
    saveWithTokenNotice: "Profile saved and key moved to the system keychain.",
    deletedNotice: "Profile deleted.",
    newDraftNotice: "Switched to a new draft.",
    openedCodexNotice: "Codex opened.",
    syncedNotice: "Local history synced.",
    restoredNotice: "Backup restored.",
    activatedPrefix: "Activated",
    activationVerifiedPrefix: "Written and verified",
    activationNeedsCheck: "Written, but the live check is not ready yet. See Status.",
    validationName: "Enter a profile name.",
    validationUrl: "Enter a valid API base URL.",
    validationModel: "Enter a model name.",
    validationGeneric: "Complete the highlighted fields first.",
    tokenRequired: "Add the API key before first activation.",
    profileSelected: "Loaded profile",
    editProfile: "Edit",
    useProfile: "Use",
    providerTag: "Provider",
    activeProfileTag: "Active profile",
    readyLabel: "Ready",
    missingLabel: "Needs input",
    keyReady: "Key ready",
    keyMissing: "Key required",
    pathLabel: "Working path",
    helperLabel: "Only these four fields matter",
    profileCountLabel: "Profiles",
    backupAction: "Restore",
    openLanguageSwitcher: "Switch language",
    themeSwitcher: "Switch theme",
  },
} as const;

function App() {
  const [preferences, setPreferences] = useState<Preferences>(() => loadPreferences());
  const [profiles, setProfiles] = useState<ProfileRecord[]>([]);
  const [draft, setDraft] = useState<CodexProfile>(emptyProfile());
  const [selectedId, setSelectedId] = useState("");
  const [token, setToken] = useState("");
  const [status, setStatus] = useState<HistoryStatus | null>(null);
  const [runtimeStatus, setRuntimeStatus] = useState<CodexRuntimeStatus | null>(null);
  const [diagnosticsReport, setDiagnosticsReport] = useState<DiagnosticsReport | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: "idle" });
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [historyRoots, setHistoryRoots] = useState<CustomHistoryRoot[]>([]);
  const [rootLabel, setRootLabel] = useState("");
  const [rootPath, setRootPath] = useState("");
  const [currentPage, setCurrentPage] = useState<AppPage>("setup");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const [syncOnActivate, setSyncOnActivate] = useState(true);
  const [restartOnActivate, setRestartOnActivate] = useState(true);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const text = copy[preferences.language];
  const showOnboarding = !preferences.onboarded;

  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.id === selectedId) || null,
    [profiles, selectedId],
  );

  const resolvedDraft = useMemo(
    () => resolveDraftProfile(draft, profiles, selectedId),
    [draft, profiles, selectedId],
  );

  const savedDraftProfile = useMemo(
    () => profiles.find((profile) => profile.id === resolvedDraft.id) || null,
    [profiles, resolvedDraft.id],
  );

  const fieldErrors = useMemo(() => validateVisibleFields(draft, text), [draft, text]);
  const profileIsValid = Object.keys(fieldErrors).length === 0;
  const secretIsReady = Boolean(savedDraftProfile?.tokenReady || token.trim());
  const activationReady = profileIsValid && secretIsReady;
  const mismatchCount = providerMismatchCount(status);
  const historyHealthy =
    Boolean(status?.ready) && status?.mismatchedProviderThreads === 0;
  const hasProfileChanges = selectedProfile
    ? comparableProfile(selectedProfile) !== comparableProfile(materializeProfile(draft, profiles, selectedId))
    : true;

  useEffect(() => {
    void refreshAll({ adoptFirstProfile: true });
  }, []);

  useEffect(() => {
    savePreferences(preferences);
    document.documentElement.setAttribute("data-theme", preferences.theme);
    document.documentElement.style.colorScheme = preferences.theme;
    document.documentElement.lang = preferences.language === "zh" ? "zh-CN" : "en";
  }, [preferences]);

  useEffect(() => {
    setError("");
    setNotice((current) => (current ? copy[preferences.language].refreshState : current));
  }, [preferences.language]);

  async function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
    try {
      setError("");
      if (!runningInTauri()) {
        return browserPreviewValue<T>(name);
      }
      return await invoke<T>(name, args);
    } catch (err) {
      const rawMessage = err instanceof Error ? err.message : String(err);
      const message = localizeCommandError(rawMessage, text);
      setError(message);
      throw new Error(message);
    }
  }

  function runningInTauri() {
    return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
  }

  function browserPreviewValue<T>(name: string): T {
    if (name === "list_profiles") return previewProfiles as T;
    if (name === "list_backups") return previewBackups as T;
    if (name === "history_status") return previewStatus as T;
    if (name === "codex_status") return previewRuntimeStatus as T;
    if (name === "diagnostics") return previewDiagnostics as T;
    if (name === "list_history_roots") return previewHistoryRoots as T;
    if (name === "save_history_root") return previewHistoryRoots as T;
    if (name === "delete_history_root") return [] as T;
    if (name === "open_codex") return undefined as T;
    if (name === "save_profile") return previewProfiles as T;
    if (name === "delete_profile") return previewProfiles.slice(0, 1) as T;
    if (name === "history_sync") {
      return {
        updatedDatabaseRows: 3,
        updatedSessionFiles: 3,
        updatedSessionIndex: true,
        updatedGlobalState: true,
      } as T;
    }
    if (name === "restore_backup") return previewBackups as T;
    if (name === "activate_profile") {
      return {
        ok: true,
        profileId: "jmrai",
        backupId: "preview",
        codexOpened: true,
        runtimeStatus: previewRuntimeStatus,
        history: {
          updatedDatabaseRows: 3,
          updatedSessionFiles: 3,
          updatedSessionIndex: true,
          updatedGlobalState: true,
        },
      } as T;
    }
    if (name === "clear_api_mode") {
      return {
        ok: true,
        backupId: "preview-clear-api-mode",
        codexOpened: true,
        runtimeStatus: {
          ...previewRuntimeStatus,
          currentProvider: "",
          providerConfigured: false,
          providerHasBearerToken: false,
          readyForRemote: false,
        },
      } as T;
    }
    throw new Error("This action is available in the desktop app.");
  }

  async function refreshAll(options: { adoptFirstProfile?: boolean; preferredId?: string; notify?: boolean } = {}) {
    setBusy(true);
    try {
      const [nextProfiles, nextStatus, nextRuntimeStatus, nextBackups, nextHistoryRoots, nextDiagnostics] = await Promise.all([
        call<ProfileRecord[]>("list_profiles"),
        call<HistoryStatus>("history_status", { codexHome: resolvedDraft.codexHome }),
        call<CodexRuntimeStatus>("codex_status", { codexHome: resolvedDraft.codexHome }),
        call<BackupRecord[]>("list_backups"),
        call<CustomHistoryRoot[]>("list_history_roots"),
        call<DiagnosticsReport>("diagnostics", { codexHome: resolvedDraft.codexHome }),
      ]);
      setProfiles(nextProfiles);
      setStatus(nextStatus);
      setRuntimeStatus(nextRuntimeStatus);
      setBackups(nextBackups);
      setHistoryRoots(nextHistoryRoots);
      setDiagnosticsReport(nextDiagnostics);

      const profileToLoad =
        nextProfiles.find((profile) => profile.id === options.preferredId) ||
        (options.adoptFirstProfile && !selectedId ? nextProfiles[0] : null);
      if (profileToLoad) {
        loadProfile(profileToLoad, false);
      }
      if (options.notify !== false) {
        setNotice(text.refreshState);
      }
    } finally {
      setBusy(false);
    }
  }

  function loadProfile(profile: ProfileRecord, announce = true) {
    setSelectedId(profile.id);
    setDraft(toCodexProfile(profile));
    setToken("");
    setShowAdvanced(Boolean(profile.codexHome));
    if (announce) {
      setNotice(`${text.profileSelected}: ${profile.name || profile.id}`);
    }
  }

  function createNewProfile() {
    const next = emptyProfile();
    setSelectedId("");
    setDraft(next);
    setToken("");
    setShowAdvanced(false);
    setNotice(text.newDraftNotice);
  }

  function editProfile(profile: ProfileRecord) {
    loadProfile(profile);
    setCurrentPage("setup");
  }

  function updateDraft<K extends keyof CodexProfile>(key: K, value: CodexProfile[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  async function saveCurrentProfile(quiet = false) {
    const errors = validateVisibleFields(draft, text);
    if (Object.keys(errors).length > 0) {
      setError(text.validationGeneric);
      throw new Error("Invalid profile");
    }

    const profile = materializeProfile(draft, profiles, selectedId);
    const nextProfiles = await call<ProfileRecord[]>("save_profile", {
      profile,
      token: token.trim() || null,
    });
    const saved = nextProfiles.find((item) => item.id === profile.id) || profile;
    setProfiles(nextProfiles);
    setDraft(toCodexProfile(saved));
    setSelectedId(saved.id);
    setToken("");
    if (!quiet) {
      setNotice(token.trim() ? text.saveWithTokenNotice : text.saveNotice);
    }
    return saved;
  }

  async function saveProfile(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      await saveCurrentProfile(false);
    } finally {
      setBusy(false);
    }
  }

  async function deleteSelectedProfile() {
    if (!selectedProfile) return;
    await deleteProfileById(selectedProfile.id);
  }

  async function deleteProfileById(id: string) {
    setBusy(true);
    try {
      const nextProfiles = await call<ProfileRecord[]>("delete_profile", { id });
      setProfiles(nextProfiles);
      if (selectedId === id && nextProfiles[0]) {
        loadProfile(nextProfiles[0], false);
        setSelectedId(nextProfiles[0].id);
      } else if (selectedId === id) {
        createNewProfile();
      }
      setNotice(text.deletedNotice);
    } finally {
      setBusy(false);
    }
  }

  async function activateCurrent() {
    const errors = validateVisibleFields(draft, text);
    if (Object.keys(errors).length > 0) {
      setError(text.validationGeneric);
      return;
    }
    if (!secretIsReady) {
      setError(text.tokenRequired);
      return;
    }

    setBusy(true);
    try {
      const activationToken = token.trim();
      const saved =
        hasProfileChanges || activationToken || !selectedProfile ? await saveCurrentProfile(true) : materializeProfile(draft, profiles, selectedId);
      const result = await call<ActivationResult>("activate_profile", {
        id: saved.id,
        options: {
          syncHistory: syncOnActivate,
          restartCodex: restartOnActivate,
          token: activationToken || null,
        },
      });
      const historyText = result.history
        ? ` ${result.history.updatedDatabaseRows} / ${result.history.updatedSessionFiles}`
        : "";
      const launchText = result.codexOpened
        ? ""
        : ` ${
            preferences.language === "zh"
              ? "Codex 未能自动打开，请从开始菜单手动打开。"
              : "Codex did not open automatically; launch it from Start or Applications."
          }`;
      if (result.runtimeStatus) {
        setRuntimeStatus(result.runtimeStatus);
      }
      setCurrentPage("status");
      await refreshAll({ preferredId: saved.id, notify: false });
      if (result.runtimeStatus?.readyForRemote) {
        setNotice(
          `${text.activationVerifiedPrefix}: ${saved.name || saved.id}.${historyText}${launchText}`,
        );
      } else {
        setNotice(`${text.activationNeedsCheck}${launchText}`);
      }
    } finally {
      setBusy(false);
    }
  }

  async function openCodex() {
    setBusy(true);
    try {
      await call<void>("open_codex");
      setNotice(text.openedCodexNotice);
    } finally {
      setBusy(false);
    }
  }

  async function clearApiMode() {
    if (typeof window !== "undefined" && !window.confirm(text.clearApiConfirm)) {
      return;
    }
    setBusy(true);
    try {
      const result = await call<ClearApiModeResult>("clear_api_mode", {
        options: {
          restartCodex: restartOnActivate,
          codexHome: resolvedDraft.codexHome,
        },
      });
      setRuntimeStatus(result.runtimeStatus);
      setNotice(`${text.clearApiBackupNotice} ${result.backupId}`);
      await refreshAll({ preferredId: resolvedDraft.id, notify: false });
    } finally {
      setBusy(false);
    }
  }

  async function checkForUpdates() {
    setUpdateStatus({ state: "checking" });
    try {
      const response = await fetch(RELEASE_API_URL, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!response.ok) {
        throw new Error(`${response.status} ${response.statusText}`);
      }
      const release = (await response.json()) as {
        tag_name?: string;
        html_url?: string;
      };
      const latestVersion = normalizeVersion(release.tag_name || "");
      const releaseUrl = release.html_url || RELEASES_URL;
      setUpdateStatus({
        state: compareVersions(latestVersion, APP_VERSION) > 0 ? "available" : "current",
        latestVersion,
        releaseUrl,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setUpdateStatus({ state: "error", message: `${text.updateFailed} ${message}` });
    }
  }

  async function copyDiagnostics() {
    const reportText = formatDiagnosticsReport(diagnosticsReport, updateStatus, preferences.language);
    try {
      await navigator.clipboard.writeText(reportText);
      setNotice(text.diagnosticsCopied);
    } catch {
      setError(reportText);
    }
  }

  async function openReleasePage() {
    const url = "releaseUrl" in updateStatus ? updateStatus.releaseUrl : RELEASES_URL;
    if (runningInTauri()) {
      await openUrl(url);
      return;
    }
    window.open(url, "_blank", "noopener,noreferrer");
  }

  async function syncHistory() {
    setBusy(true);
    try {
      await call<{ updatedDatabaseRows: number; updatedSessionFiles: number }>("history_sync", {
        codexHome: resolvedDraft.codexHome,
      });
      setNotice(text.syncedNotice);
      await refreshAll({ preferredId: resolvedDraft.id, notify: false });
    } finally {
      setBusy(false);
    }
  }

  async function saveHistoryRoot(event: FormEvent) {
    event.preventDefault();
    if (!rootPath.trim()) {
      setError(text.fieldRootPath);
      return;
    }
    setBusy(true);
    try {
      const nextRoots = await call<CustomHistoryRoot[]>("save_history_root", {
        path: rootPath.trim(),
        label: rootLabel.trim() || null,
      });
      setHistoryRoots(nextRoots);
      setRootLabel("");
      setRootPath("");
      setNotice(text.saveNotice);
    } finally {
      setBusy(false);
    }
  }

  async function deleteHistoryRoot(id: string) {
    setBusy(true);
    try {
      const nextRoots = await call<CustomHistoryRoot[]>("delete_history_root", { id });
      setHistoryRoots(nextRoots);
      setNotice(text.deletedNotice);
    } finally {
      setBusy(false);
    }
  }

  async function restoreBackup(id: string) {
    setBusy(true);
    try {
      const nextBackups = await call<BackupRecord[]>("restore_backup", { id });
      setBackups(nextBackups);
      setNotice(`${text.restoredNotice} ${id}`);
      await refreshAll({ preferredId: resolvedDraft.id, notify: false });
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app-shell" aria-busy={busy}>
      <AmbientHalo theme={preferences.theme} />

      <div className="tech-label tech-top-left">{text.topTech}</div>
      <section className={`layout-grid ${showOnboarding ? "layout-grid-onboarding" : ""}`}>
        <div className={`hero-column ${showOnboarding ? "hero-column-onboarding" : ""}`}>
          <div className="hero-copy">
            <div className="brand-badge">
              <Sparkle size={14} weight="bold" />
              <span>{text.appName}</span>
            </div>
            <h1>
              {text.heroTitle.split("\n").map((line) => (
                <span key={line}>{line}</span>
              ))}
            </h1>
            <p>{text.heroBody}</p>
          </div>

          {!showOnboarding && (
            <>
              <div className="step-list" aria-label={text.helperLabel}>
                {text.steps.map((item, index) => (
                  <article className="step-card" key={item}>
                    <small>{String(index + 1).padStart(2, "0")}</small>
                    <strong>{item}</strong>
                  </article>
                ))}
              </div>

              <div className="support-grid">
                <section className="support-card summary-card">
                  <div className="support-card-header">
                    <div>
                      <small>{text.profilesTitle}</small>
                      <strong>{profiles.length}</strong>
                    </div>
                    <button
                      className="mini-ghost"
                      type="button"
                      onClick={() => {
                        createNewProfile();
                        setCurrentPage("setup");
                      }}
                    >
                      <Plus size={15} weight="bold" />
                    </button>
                  </div>
                  <div className="inline-note">
                    <Info size={14} weight="bold" />
                    <span>{profiles.length === 0 ? text.profilesEmpty : text.profilesSummary}</span>
                  </div>
                  <button className="support-action" type="button" onClick={() => setCurrentPage("profiles")}>
                    <FolderSimple size={15} weight="bold" />
                    {text.manageProfiles}
                  </button>
                </section>

                <section className="support-card">
                  <div className="support-card-header">
                    <div>
                      <small>{text.localState}</small>
                      <strong>
                        {status ? (historyHealthy ? text.historyAligned : text.historyNeedsSync) : text.historyChecking}
                      </strong>
                    </div>
                    <button className="mini-ghost" type="button" onClick={() => refreshAll()} disabled={busy}>
                      <ArrowClockwise size={15} weight="bold" />
                    </button>
                  </div>
                  {status ? (
                    <>
                      <dl className="metric-list">
                        <Metric label={text.providerLabel} value={status.currentProvider || "-"} />
                        <Metric label={text.modelLabel} value={status.currentModel || "-"} />
                        <Metric label={text.threadsLabel} value={String(status.totalThreads)} />
                        <Metric label={text.mismatchLabel} value={String(mismatchCount)} />
                      </dl>
                      {status.reason && (
                        <div className="inline-note">
                          <Info size={14} weight="bold" />
                          <span>{status.reason}</span>
                        </div>
                      )}
                      <div className="inline-note">
                        <Info size={14} weight="bold" />
                        <span>{text.historyHint}</span>
                      </div>
                      <button className="support-action" type="button" onClick={syncHistory} disabled={busy}>
                        <Database size={15} weight="bold" />
                        {text.historySync}
                      </button>
                    </>
                  ) : (
                    <div className="skeleton-stack">
                      <span />
                      <span />
                    </div>
                  )}
                </section>

                <section className="support-card">
                  <div className="support-card-header">
                    <div>
                      <small>{text.backupsTitle}</small>
                      <strong>{backups.length}</strong>
                    </div>
                    <Archive size={16} weight="duotone" />
                  </div>
                  <div className="backup-list">
                    {backups.length === 0 ? (
                      <div className="empty-inline">{text.backupsEmpty}</div>
                    ) : (
                      backups.slice(0, 3).map((backup) => (
                        <button
                          className="backup-row"
                          type="button"
                          key={backup.id}
                          onClick={() => restoreBackup(backup.id)}
                          disabled={busy}
                        >
                          <span>
                            <strong>{backup.label || backup.id}</strong>
                            <small>{formatDate(backup.createdAt, preferences.language)}</small>
                          </span>
                          <ClockCounterClockwise size={15} weight="bold" />
                        </button>
                      ))
                    )}
                  </div>
                </section>
              </div>
            </>
          )}
        </div>

        <section className={`panel-column ${showOnboarding ? "panel-column-onboarding" : ""}`}>
          <div className="auth-panel">
            <div className="top-bar">
              <div className="toolbar-cluster toolbar-language" role="group" aria-label={text.openLanguageSwitcher}>
                <span className="version-tag">V0.1</span>
                <button
                  className={`toolbar-pill ${preferences.language === "zh" ? "active" : ""}`}
                  type="button"
                  onClick={() => setPreferences((current) => ({ ...current, language: "zh" }))}
                >
                  中
                </button>
                <button
                  className={`toolbar-pill ${preferences.language === "en" ? "active" : ""}`}
                  type="button"
                  onClick={() => setPreferences((current) => ({ ...current, language: "en" }))}
                >
                  EN
                </button>
              </div>
              <button
                className="toolbar-theme"
                type="button"
                onClick={() =>
                  setPreferences((current) => ({
                    ...current,
                    theme: current.theme === "dark" ? "light" : "dark",
                  }))
                }
                aria-label={text.themeSwitcher}
              >
                {preferences.theme === "dark" ? <Moon size={16} weight="bold" /> : <Sun size={16} weight="bold" />}
                <span>{preferences.theme === "dark" ? text.darkTheme : text.lightTheme}</span>
              </button>
            </div>

            {!showOnboarding && (
              <nav className="page-tabs" aria-label="Remote Codex API sections">
                <button
                  className={currentPage === "setup" ? "active" : ""}
                  type="button"
                  onClick={() => setCurrentPage("setup")}
                >
                  <Lightning size={14} weight="bold" />
                  {text.pageSetup}
                </button>
                <button
                  className={currentPage === "profiles" ? "active" : ""}
                  type="button"
                  onClick={() => setCurrentPage("profiles")}
                >
                  <FolderSimple size={14} weight="bold" />
                  {text.pageProfiles}
                </button>
                <button
                  className={currentPage === "history" ? "active" : ""}
                  type="button"
                  onClick={() => setCurrentPage("history")}
                >
                  <Database size={14} weight="bold" />
                  {text.pageHistory}
                </button>
                <button
                  className={currentPage === "guide" ? "active" : ""}
                  type="button"
                  onClick={() => setCurrentPage("guide")}
                >
                  <Info size={14} weight="bold" />
                  {text.pageGuide}
                </button>
                <button
                  className={currentPage === "status" ? "active" : ""}
                  type="button"
                  onClick={() => setCurrentPage("status")}
                >
                  <CheckCircle size={14} weight="bold" />
                  {text.pageStatus}
                </button>
              </nav>
            )}

            {showOnboarding ? (
              <>
                <div className="panel-header">
                  <div className="system-badge">{text.chooseLanguage}</div>
                  <h2>{text.languageLabel}</h2>
                  <p>{text.chooseLanguageBody}</p>
                </div>

                <div className="choice-grid">
                  <ChoiceCard
                    title={text.chinese}
                    detail={text.chineseHint}
                    active={preferences.language === "zh"}
                    onClick={() => setPreferences((current) => ({ ...current, language: "zh" }))}
                    icon={<Globe size={18} weight="bold" />}
                  />
                  <ChoiceCard
                    title={text.english}
                    detail={text.englishHint}
                    active={preferences.language === "en"}
                    onClick={() => setPreferences((current) => ({ ...current, language: "en" }))}
                    icon={<Globe size={18} weight="bold" />}
                  />
                </div>

                <div className="panel-header compact-head">
                  <div className="system-badge">{text.chooseTheme}</div>
                  <p>{text.chooseThemeBody}</p>
                </div>

                <div className="choice-grid">
                  <ChoiceCard
                    title={text.darkTheme}
                    detail={text.darkHint}
                    active={preferences.theme === "dark"}
                    onClick={() => setPreferences((current) => ({ ...current, theme: "dark" }))}
                    icon={<Moon size={18} weight="bold" />}
                  />
                  <ChoiceCard
                    title={text.lightTheme}
                    detail={text.lightHint}
                    active={preferences.theme === "light"}
                    onClick={() => setPreferences((current) => ({ ...current, theme: "light" }))}
                    icon={<Sun size={18} weight="bold" />}
                  />
                </div>

                <button
                  className="primary-button large"
                  type="button"
                  onClick={() => setPreferences((current) => ({ ...current, onboarded: true }))}
                >
                  {text.continue}
                  <RocketLaunch size={18} weight="bold" />
                </button>
              </>
            ) : currentPage === "setup" ? (
              <form className="panel-form" onSubmit={saveProfile}>
                <div className="panel-header">
                  <div className="system-badge">{text.quickPanelBadge}</div>
                  <h2>{draft.name.trim() || text.quickPanelTitle}</h2>
                  <p>{text.quickPanelBody}</p>
                </div>

                <div className="status-row">
                  <StatusPill label={profileIsValid ? text.readyLabel : text.missingLabel} tone={profileIsValid ? "good" : "warn"} />
                  <StatusPill label={secretIsReady ? text.keyReady : text.keyMissing} tone={secretIsReady ? "good" : "warn"} />
                  <StatusPill label={historyHealthy ? text.historyAligned : text.historyNeedsSync} tone={historyHealthy ? "good" : "idle"} />
                </div>

                <div className="field-stack">
                  <Field
                    label={text.fieldName}
                    helper={text.fieldNameHelper}
                    error={fieldErrors.name}
                    value={draft.name}
                    onChange={(value) => updateDraft("name", value)}
                    placeholder="JMRAI"
                  />
                  <Field
                    label={text.fieldUrl}
                    helper={text.fieldUrlHelper}
                    error={fieldErrors.baseUrl}
                    value={draft.baseUrl}
                    onChange={(value) => updateDraft("baseUrl", value)}
                    placeholder="https://provider.example/v1"
                  />
                  <Field
                    label={text.fieldModel}
                    helper={text.fieldModelHelper}
                    error={fieldErrors.model}
                    value={draft.model}
                    onChange={(value) => updateDraft("model", value)}
                    placeholder="gpt-5.5"
                  />
                  <Field
                    label={text.fieldToken}
                    helper={savedDraftProfile?.tokenReady ? text.fieldTokenHelperExisting : text.fieldTokenHelperNew}
                    value={token}
                    onChange={setToken}
                    placeholder="sk-..."
                    type="password"
                    tone={!secretIsReady ? "warn" : "default"}
                  />
                </div>

                <div className="pill-grid">
                  <StaticPill icon={<CheckCircle size={16} weight="bold" />} label={text.keepLogin} />
                  <StaticPill icon={<LockKey size={16} weight="bold" />} label={text.keychainSaved} />
                </div>

                <div className="toggle-block">
                  <div className="toggle-header">
                    <span>{text.launchOptions}</span>
                    <Lightning size={16} weight="bold" />
                  </div>
                  <ToggleRow
                    label={text.syncOnActivate}
                    checked={syncOnActivate}
                    onChange={setSyncOnActivate}
                    icon={<Database size={16} weight="bold" />}
                  />
                  <ToggleRow
                    label={text.restartOnActivate}
                    checked={restartOnActivate}
                    onChange={setRestartOnActivate}
                    icon={<TerminalWindow size={16} weight="bold" />}
                  />
                </div>

                <button
                  className="advanced-toggle"
                  type="button"
                  onClick={() => setShowAdvanced((current) => !current)}
                >
                  <span>{text.advanced}</span>
                  {showAdvanced ? <CaretUp size={16} weight="bold" /> : <CaretDown size={16} weight="bold" />}
                </button>

                {showAdvanced && (
                  <div className="advanced-panel">
                    <Field
                      label={text.fieldCodexHome}
                      helper={text.fieldCodexHomeHelper}
                      value={draft.codexHome || ""}
                      onChange={(value) => updateDraft("codexHome", value)}
                      placeholder="~/.codex"
                    />
                    <div className="derived-row">
                      <small>{text.providerTag}</small>
                      <strong>{resolvedDraft.providerName}</strong>
                    </div>
                    <div className="derived-row">
                      <small>{text.pathLabel}</small>
                      <strong>{resolvedDraft.codexHome || "~/.codex"}</strong>
                    </div>
                  </div>
                )}

                <div className="action-stack">
                  <button className="primary-button large" type="button" onClick={activateCurrent} disabled={busy || !activationReady}>
                    {text.saveActivate}
                    <RocketLaunch size={18} weight="bold" />
                  </button>

                  <div className="secondary-row">
                    <button className="secondary-button" type="submit" disabled={busy || !profileIsValid}>
                      <FloppyDisk size={16} weight="bold" />
                      {text.saveOnly}
                    </button>
                    <button className="secondary-button" type="button" onClick={openCodex} disabled={busy}>
                      <TerminalWindow size={16} weight="bold" />
                      {text.openCodex}
                    </button>
                  </div>

                  <div className="foot-actions">
                    <button className="mini-ghost" type="button" onClick={() => refreshAll()} disabled={busy}>
                      <ArrowClockwise size={15} weight="bold" />
                      {text.refreshState}
                    </button>
                    {selectedProfile && (
                      <button className="danger-link" type="button" onClick={deleteSelectedProfile} disabled={busy}>
                        <Trash size={15} weight="bold" />
                        {text.deleteProfile}
                      </button>
                    )}
                  </div>
                </div>
              </form>
            ) : currentPage === "profiles" ? (
              <ProfilesPage
                text={text}
                profiles={profiles}
                selectedId={selectedId}
                busy={busy}
                onNew={() => {
                  createNewProfile();
                  setCurrentPage("setup");
                }}
                onEdit={editProfile}
                onDelete={deleteProfileById}
              />
            ) : currentPage === "history" ? (
              <HistoryPage
                text={text}
                status={status}
                historyRoots={historyRoots}
                rootLabel={rootLabel}
                rootPath={rootPath}
                onRootLabelChange={setRootLabel}
                onRootPathChange={setRootPath}
                onSaveRoot={saveHistoryRoot}
                onDeleteRoot={deleteHistoryRoot}
                onSyncHistory={syncHistory}
                busy={busy}
              />
            ) : currentPage === "guide" ? (
              <GuidePage text={text} />
            ) : (
              <RuntimeStatusPage
                text={text}
                status={runtimeStatus}
                historyStatus={status}
                diagnosticsReport={diagnosticsReport}
                updateStatus={updateStatus}
                backups={backups}
                mismatchCount={mismatchCount}
                onRefresh={() => refreshAll()}
                onSyncHistory={syncHistory}
                onClearApiMode={clearApiMode}
                onCheckUpdates={checkForUpdates}
                onCopyDiagnostics={copyDiagnostics}
                onOpenReleases={openReleasePage}
                onRestoreBackup={restoreBackup}
                busy={busy}
                language={preferences.language}
              />
            )}
          </div>

          {!showOnboarding && (
            <footer className={`notice-dock ${error ? "has-error" : ""}`}>
              <span className="notice-main">
                {error ? <XCircle size={16} weight="bold" /> : busy ? <ArrowClockwise size={16} weight="bold" /> : <CheckCircle size={16} weight="bold" />}
                {error || notice || text.appSubtitle}
              </span>
              <span className="notice-meta">
                <FolderSimple size={15} weight="bold" />
                {resolvedDraft.codexHome || "~/.codex"}
              </span>
            </footer>
          )}
        </section>
      </section>
    </main>
  );
}

function emptyProfile(): CodexProfile {
  const now = new Date().toISOString();
  return {
    id: "",
    name: "",
    providerName: "",
    baseUrl: "https://",
    model: "gpt-5.5",
    requiresOpenaiAuth: true,
    keepChatgptLogin: true,
    codexHome: "",
    createdAt: now,
    updatedAt: now,
  };
}

function detectLanguage(): Language {
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) {
    return "zh";
  }
  return "en";
}

function detectTheme(): Theme {
  if (typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: light)").matches) {
    return "light";
  }
  return "dark";
}

function loadPreferences(): Preferences {
  const defaults: Preferences = {
    language: detectLanguage(),
    theme: detectTheme(),
    onboarded: false,
  };
  if (typeof window === "undefined") return defaults;
  try {
    const raw = window.localStorage.getItem(PREFERENCES_STORAGE_KEY);
    if (!raw) return defaults;
    const parsed = JSON.parse(raw) as Partial<Preferences>;
    const onboarded = Boolean(parsed.onboarded);
    return {
      language: parsed.language === "zh" || parsed.language === "en" ? parsed.language : defaults.language,
      theme: onboarded && (parsed.theme === "light" || parsed.theme === "dark") ? parsed.theme : defaults.theme,
      onboarded,
    };
  } catch {
    return defaults;
  }
}

function toCodexProfile(profile: CodexProfile | ProfileRecord): CodexProfile {
  return {
    id: profile.id,
    name: profile.name,
    providerName: profile.providerName,
    baseUrl: profile.baseUrl,
    model: profile.model,
    requiresOpenaiAuth: profile.requiresOpenaiAuth,
    keepChatgptLogin: profile.keepChatgptLogin,
    codexHome: profile.codexHome ?? null,
    createdAt: profile.createdAt,
    updatedAt: profile.updatedAt,
  };
}

function savePreferences(preferences: Preferences) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(PREFERENCES_STORAGE_KEY, JSON.stringify(preferences));
  } catch {
    // Ignore persistence failures in preview or locked browser contexts.
  }
}

function validateVisibleFields(
  profile: CodexProfile,
  text: (typeof copy)["zh"] | (typeof copy)["en"],
): VisibleFieldErrors {
  const errors: VisibleFieldErrors = {};
  if (!profile.name.trim()) errors.name = text.validationName;

  const baseUrl = profile.baseUrl.trim();
  if (!baseUrl) {
    errors.baseUrl = text.validationUrl;
  } else {
    try {
      const url = new URL(baseUrl);
      if (url.protocol !== "https:" && url.protocol !== "http:") {
        errors.baseUrl = text.validationUrl;
      }
    } catch {
      errors.baseUrl = text.validationUrl;
    }
  }

  if (!profile.model.trim()) errors.model = text.validationModel;
  return errors;
}

function parseHost(baseUrl: string) {
  try {
    return new URL(baseUrl).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

function prettifyHost(host: string) {
  return host
    .split(".")[0]
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .trim();
}

function slugify(value: string) {
  return value
    .normalize("NFKD")
    .replace(/[^\x00-\x7F]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 36);
}

function uniqueProfileId(baseId: string, profiles: CodexProfile[], selectedId: string) {
  const fallback = baseId || "provider";
  const occupied = new Set(profiles.filter((profile) => profile.id !== selectedId).map((profile) => profile.id));
  if (!occupied.has(fallback)) return fallback;

  let index = 2;
  while (occupied.has(`${fallback}-${index}`)) {
    index += 1;
  }
  return `${fallback}-${index}`;
}

function resolveDraftProfile(draft: CodexProfile, profiles: CodexProfile[], selectedId: string) {
  const baseUrl = draft.baseUrl.trim();
  const host = parseHost(baseUrl);
  const name = draft.name.trim();
  const providerName = name || prettifyHost(host) || draft.providerName.trim() || "Provider";
  const incomingId = draft.id.trim();
  const generatedId = slugify(incomingId || providerName || host || "provider") || "provider";
  const id = incomingId ? generatedId : uniqueProfileId(generatedId, profiles, selectedId);

  return {
    id,
    name,
    providerName,
    baseUrl,
    model: draft.model.trim(),
    codexHome: draft.codexHome?.trim() || null,
  };
}

function materializeProfile(draft: CodexProfile, profiles: CodexProfile[], selectedId: string): CodexProfile {
  const resolved = resolveDraftProfile(draft, profiles, selectedId);
  const now = new Date().toISOString();
  return {
    ...draft,
    id: resolved.id,
    name: resolved.name,
    providerName: resolved.providerName,
    baseUrl: resolved.baseUrl,
    model: resolved.model,
    codexHome: resolved.codexHome,
    requiresOpenaiAuth: true,
    keepChatgptLogin: true,
    createdAt: draft.createdAt || now,
    updatedAt: now,
  };
}

function comparableProfile(profile: CodexProfile) {
  return JSON.stringify({
    id: profile.id.trim(),
    name: profile.name.trim(),
    providerName: profile.providerName.trim(),
    baseUrl: profile.baseUrl.trim(),
    model: profile.model.trim(),
    codexHome: profile.codexHome?.trim() || null,
  });
}

function providerMismatchCount(status: HistoryStatus | null) {
  return status ? status.mismatchedProviderThreads : 0;
}

function localizeCommandError(
  message: string,
  text: (typeof copy)["zh"] | (typeof copy)["en"],
) {
  if (message.toLowerCase().includes("missing provider token")) {
    return text.tokenRequired;
  }
  return message;
}

function formatDate(value: string, language: Language) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en-US", {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function ProfilesPage({
  text,
  profiles,
  selectedId,
  busy,
  onNew,
  onEdit,
  onDelete,
}: {
  text: (typeof copy)["zh"] | (typeof copy)["en"];
  profiles: ProfileRecord[];
  selectedId: string;
  busy: boolean;
  onNew: () => void;
  onEdit: (profile: ProfileRecord) => void;
  onDelete: (id: string) => void;
}) {
  return (
    <section className="panel-page profiles-page">
      <div className="panel-header">
        <div className="system-badge">{text.pageProfiles}</div>
        <h2>{text.profilesTitle}</h2>
        <p>{text.profilesSummary}</p>
      </div>

      <button className="primary-button large" type="button" onClick={onNew} disabled={busy}>
        {text.newProfile}
        <Plus size={18} weight="bold" />
      </button>

      <div className="managed-profile-list">
        {profiles.length === 0 ? (
          <div className="empty-state">{text.profilesEmpty}</div>
        ) : (
          profiles.map((profile) => (
            <article className={`managed-profile-card ${profile.id === selectedId ? "selected" : ""}`} key={profile.id}>
              <div className="managed-profile-main">
                <div>
                  <small>{profile.tokenReady ? text.keyReady : text.keyMissing}</small>
                  <strong>{profile.name || profile.id}</strong>
                </div>
                <span>{profile.model}</span>
              </div>
              <code>{profile.baseUrl}</code>
              <div className="managed-profile-actions">
                <button className="secondary-button" type="button" onClick={() => onEdit(profile)} disabled={busy}>
                  <FloppyDisk size={15} weight="bold" />
                  {text.editProfile}
                </button>
                <button className="danger-link" type="button" onClick={() => onDelete(profile.id)} disabled={busy}>
                  <Trash size={15} weight="bold" />
                  {text.deleteProfile}
                </button>
              </div>
            </article>
          ))
        )}
      </div>
    </section>
  );
}

function HistoryPage({
  text,
  status,
  historyRoots,
  rootLabel,
  rootPath,
  onRootLabelChange,
  onRootPathChange,
  onSaveRoot,
  onDeleteRoot,
  onSyncHistory,
  busy,
}: {
  text: (typeof copy)["zh"] | (typeof copy)["en"];
  status: HistoryStatus | null;
  historyRoots: CustomHistoryRoot[];
  rootLabel: string;
  rootPath: string;
  onRootLabelChange: (value: string) => void;
  onRootPathChange: (value: string) => void;
  onSaveRoot: (event: FormEvent) => void;
  onDeleteRoot: (id: string) => void;
  onSyncHistory: () => void;
  busy: boolean;
}) {
  return (
    <section className="panel-page history-page">
      <div className="panel-header">
        <div className="system-badge">{text.pageHistory}</div>
        <h2>{text.historyPageTitle}</h2>
        <p>{text.historyPageIntro}</p>
      </div>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.localState}</small>
            <strong>{status?.ready ? text.historyAligned : text.historyChecking}</strong>
          </div>
          <Database size={16} weight="bold" />
        </div>
        <dl className="metric-list">
          <Metric label={text.threadsLabel} value={status ? String(status.totalThreads) : "-"} />
          <Metric label={text.mismatchLabel} value={status ? String(status.mismatchedProviderThreads) : "-"} />
          <Metric label={text.sessionsLabel} value={status ? String(status.sessionFileCount) : "-"} />
          <Metric label={text.indexLabel} value={status ? String(status.sessionIndexCount) : "-"} />
        </dl>
        <button className="support-action" type="button" onClick={onSyncHistory} disabled={busy}>
          <Database size={15} weight="bold" />
          {text.historySync}
        </button>
      </section>

      <form className="write-card root-form" onSubmit={onSaveRoot}>
        <div className="support-card-header">
          <div>
            <small>{text.historyRootTitle}</small>
            <strong>{historyRoots.length}</strong>
          </div>
          <FolderSimple size={16} weight="bold" />
        </div>
        <p className="section-copy">{text.historyRootIntro}</p>
        <Field
          label={text.fieldRootLabel}
          value={rootLabel}
          onChange={onRootLabelChange}
          placeholder="WSL Ubuntu"
        />
        <Field
          label={text.fieldRootPath}
          value={rootPath}
          onChange={onRootPathChange}
          placeholder="\\\\wsl.localhost\\Ubuntu\\home\\user\\project"
        />
        <button className="primary-button" type="submit" disabled={busy || !rootPath.trim()}>
          {text.addRoot}
          <Plus size={17} weight="bold" />
        </button>
      </form>

      <div className="root-list">
        {historyRoots.length === 0 ? (
          <div className="empty-state">{text.rootsEmpty}</div>
        ) : (
          historyRoots.map((root) => (
            <article className="root-card" key={root.id}>
              <span>
                <strong>{root.label}</strong>
                <code>{root.path}</code>
              </span>
              <button className="danger-link" type="button" onClick={() => onDeleteRoot(root.id)} disabled={busy}>
                <Trash size={15} weight="bold" />
                {text.deleteRoot}
              </button>
            </article>
          ))
        )}
      </div>
    </section>
  );
}

function GuidePage({ text }: { text: (typeof copy)["zh"] | (typeof copy)["en"] }) {
  return (
    <section className="panel-page guide-page">
      <div className="panel-header">
        <div className="system-badge">{text.guideBadge}</div>
        <h2>{text.guideTitle}</h2>
        <p>{text.guideIntro}</p>
      </div>

      <div className="guide-steps">
        {text.guideSteps.map((step, index) => (
          <article className="guide-step" key={step}>
            <span>{String(index + 1).padStart(2, "0")}</span>
            <p>{step}</p>
          </article>
        ))}
      </div>

      <section className="write-card input-guide-card">
        <div className="support-card-header">
          <div>
            <small>{text.fieldsTitle}</small>
            <strong>{text.helperLabel}</strong>
          </div>
          <FloppyDisk size={16} weight="bold" />
        </div>
        <div className="field-guide-grid">
          {text.fieldGuideRows.map(([label, detail]) => (
            <div className="field-guide-row" key={label}>
              <strong>{label}</strong>
              <span>{detail}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.writesTitle}</small>
            <strong>auth.json / config.toml</strong>
          </div>
          <LockKey size={16} weight="bold" />
        </div>
        <div className="write-list">
          <div className="inline-note">
            <CheckCircle size={14} weight="bold" />
            <span>{text.writesAuth}</span>
          </div>
          <div className="inline-note">
            <CheckCircle size={14} weight="bold" />
            <span>{text.writesConfig}</span>
          </div>
        </div>
      </section>

      <div className="guide-warning">
        <Info size={15} weight="bold" />
        <span>{text.guideWarning}</span>
      </div>
    </section>
  );
}

function RuntimeStatusPage({
  text,
  status,
  historyStatus,
  diagnosticsReport,
  updateStatus,
  backups,
  mismatchCount,
  onRefresh,
  onSyncHistory,
  onClearApiMode,
  onCheckUpdates,
  onCopyDiagnostics,
  onOpenReleases,
  onRestoreBackup,
  busy,
  language,
}: {
  text: (typeof copy)["zh"] | (typeof copy)["en"];
  status: CodexRuntimeStatus | null;
  historyStatus: HistoryStatus | null;
  diagnosticsReport: DiagnosticsReport | null;
  updateStatus: UpdateStatus;
  backups: BackupRecord[];
  mismatchCount: number;
  onRefresh: () => void;
  onSyncHistory: () => void;
  onClearApiMode: () => void;
  onCheckUpdates: () => void;
  onCopyDiagnostics: () => void;
  onOpenReleases: () => void;
  onRestoreBackup: (id: string) => void;
  busy: boolean;
  language: Language;
}) {
  const ready = Boolean(status?.readyForRemote);
  const updateMessage = updateStatusMessage(updateStatus, text);
  return (
    <section className="panel-page status-page">
      <div className="panel-header">
        <div className="system-badge">{text.statusBadge}</div>
        <h2>{text.statusTitle}</h2>
        <p>{text.statusIntro}</p>
      </div>

      <div className={`runtime-banner ${ready ? "ready" : "blocked"}`}>
        {ready ? <CheckCircle size={18} weight="bold" /> : <XCircle size={18} weight="bold" />}
        <strong>{ready ? text.remoteReady : text.remoteNotReady}</strong>
      </div>

      <div className="runtime-checks">
        <CheckRow label={text.authModeLabel} ok={Boolean(status?.authModeChatgpt)} />
        <CheckRow label={text.apiKeyNullLabel} ok={Boolean(status?.openaiApiKeyNull)} />
        <CheckRow label={text.providerBlockLabel} ok={Boolean(status?.providerConfigured)} />
        <CheckRow label={text.requiresAuthLabel} ok={Boolean(status?.providerRequiresOpenaiAuth)} />
        <CheckRow label={text.bearerTokenLabel} ok={Boolean(status?.providerHasBearerToken)} />
      </div>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.currentConfigTitle}</small>
            <strong>{status?.currentProvider || "-"}</strong>
          </div>
          <button className="mini-ghost" type="button" onClick={onRefresh} disabled={busy}>
            <ArrowClockwise size={15} weight="bold" />
          </button>
        </div>
        <dl className="metric-list">
          <Metric label={text.providerLabel} value={status?.currentProvider || "-"} />
          <Metric label={text.apiProviderLabel} value={status?.providerName || "-"} />
          <Metric label={text.modelLabel} value={status?.currentModel || "-"} />
          <Metric label={text.threadsLabel} value={historyStatus ? String(historyStatus.totalThreads) : "-"} />
          <Metric label={text.mismatchLabel} value={historyStatus ? String(mismatchCount) : "-"} />
        </dl>
        <div className="inline-note">
          <Info size={14} weight="bold" />
          <span>{text.statusRefreshHint}</span>
        </div>
      </section>

      <div className="status-actions">
        <button className="support-action" type="button" onClick={onSyncHistory} disabled={busy}>
          <Database size={15} weight="bold" />
          {text.historySync}
        </button>
        {backups[0] && (
          <button className="secondary-button" type="button" onClick={() => onRestoreBackup(backups[0].id)} disabled={busy}>
            <ClockCounterClockwise size={15} weight="bold" />
            {text.backupAction} {formatDate(backups[0].createdAt, language)}
          </button>
        )}
      </div>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.clearApiTitle}</small>
            <strong>{text.clearApiMode}</strong>
          </div>
          <LockKey size={16} weight="bold" />
        </div>
        <p className="section-copy">{text.clearApiIntro}</p>
        <button className="secondary-button full-width" type="button" onClick={onClearApiMode} disabled={busy}>
          <ClockCounterClockwise size={15} weight="bold" />
          {text.clearApiMode}
        </button>
      </section>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.updateTitle}</small>
            <strong>{updateMessage}</strong>
          </div>
          <ArrowClockwise size={16} weight="bold" />
        </div>
        <p className="section-copy">{text.updateIntro}</p>
        <dl className="metric-list">
          <Metric label={text.currentVersion} value={APP_VERSION} />
          <Metric
            label={text.latestVersion}
            value={"latestVersion" in updateStatus ? updateStatus.latestVersion : "-"}
          />
        </dl>
        <div className="status-actions">
          <button className="support-action" type="button" onClick={onCheckUpdates} disabled={busy || updateStatus.state === "checking"}>
            <ArrowClockwise size={15} weight="bold" />
            {text.updateCheck}
          </button>
          <button className="secondary-button" type="button" onClick={onOpenReleases}>
            {text.updateOpen}
          </button>
        </div>
      </section>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.diagnosticsTitle}</small>
            <strong>{diagnosticsReport?.platform || "-"}</strong>
          </div>
          <TerminalWindow size={16} weight="bold" />
        </div>
        <p className="section-copy">{text.diagnosticsIntro}</p>
        <dl className="metric-list">
          <Metric label={text.diagnosticsInstall} value={diagnosticsReport?.codexInstallFound ? text.readyLabel : text.missingLabel} />
          <Metric label={text.diagnosticsProcess} value={diagnosticsReport ? String(diagnosticsReport.codexProcessCount) : "-"} />
          <Metric label={text.diagnosticsProfiles} value={diagnosticsReport ? String(diagnosticsReport.profileCount) : "-"} />
          <Metric label={text.diagnosticsBackups} value={diagnosticsReport ? String(diagnosticsReport.backupCount) : "-"} />
          <Metric label={text.diagnosticsRoots} value={diagnosticsReport ? String(diagnosticsReport.customRootCount) : "-"} />
        </dl>
        <div className="diagnostic-file-grid">
          {diagnosticFileRows(diagnosticsReport, text).map((item) => (
            <span className={item.ok ? "ok" : "missing"} key={item.label}>
              {item.label}
            </span>
          ))}
        </div>
        <button className="secondary-button full-width" type="button" onClick={onCopyDiagnostics} disabled={busy || !diagnosticsReport}>
          <FloppyDisk size={15} weight="bold" />
          {text.diagnosticsCopy}
        </button>
      </section>

      <section className="write-card">
        <div className="support-card-header">
          <div>
            <small>{text.compatibilityTitle}</small>
            <strong>Codex Mate</strong>
          </div>
          <Info size={16} weight="bold" />
        </div>
        <p className="section-copy">{text.compatibilityIntro}</p>
      </section>
    </section>
  );
}

function CheckRow({ label, ok }: { label: string; ok: boolean }) {
  return (
    <div className={`check-row ${ok ? "ok" : "missing"}`}>
      {ok ? <CheckCircle size={16} weight="bold" /> : <XCircle size={16} weight="bold" />}
      <span>{label}</span>
    </div>
  );
}

function updateStatusMessage(
  status: UpdateStatus,
  text: (typeof copy)["zh"] | (typeof copy)["en"],
) {
  if (status.state === "checking") return text.updateCheck;
  if (status.state === "available") return `${text.updateAvailable}: ${status.latestVersion}`;
  if (status.state === "current") return text.updateCurrent;
  if (status.state === "error") return status.message;
  return text.currentVersion;
}

function normalizeVersion(value: string) {
  return value.trim().replace(/^v/i, "") || "0.0.0";
}

function compareVersions(left: string, right: string) {
  const leftParts = normalizeVersion(left).split(/[^0-9]+/).map((part) => Number(part || 0));
  const rightParts = normalizeVersion(right).split(/[^0-9]+/).map((part) => Number(part || 0));
  const size = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < size; index += 1) {
    const diff = (leftParts[index] || 0) - (rightParts[index] || 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

function diagnosticFileRows(
  report: DiagnosticsReport | null,
  text: (typeof copy)["zh"] | (typeof copy)["en"],
) {
  return [
    { label: `${text.diagnosticsFiles}: config.toml`, ok: Boolean(report?.configExists) },
    { label: "auth.json", ok: Boolean(report?.authExists) },
    { label: "state_5.sqlite", ok: Boolean(report?.databaseExists) },
    { label: "sessions", ok: Boolean(report?.sessionsExists) },
    { label: "session_index.jsonl", ok: Boolean(report?.sessionIndexExists) },
    { label: ".codex-global-state.json", ok: Boolean(report?.globalStateExists) },
  ];
}

function formatDiagnosticsReport(
  report: DiagnosticsReport | null,
  updateStatus: UpdateStatus,
  language: Language,
) {
  return JSON.stringify(
    {
      product: "Remote Codex API",
      locale: language,
      generatedAt: new Date().toISOString(),
      updateStatus,
      diagnostics: report,
      redaction: "No bearer token or API key is included in this report.",
    },
    null,
    2,
  );
}

function ChoiceCard({
  title,
  detail,
  active,
  onClick,
  icon,
}: {
  title: string;
  detail: string;
  active: boolean;
  onClick: () => void;
  icon: ReactNode;
}) {
  return (
    <button className={`choice-card ${active ? "active" : ""}`} type="button" onClick={onClick}>
      <span className="choice-icon">{icon}</span>
      <strong>{title}</strong>
      <small>{detail}</small>
    </button>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
  helper,
  error,
  type = "text",
  tone = "default",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  helper?: string;
  error?: string;
  type?: string;
  tone?: "default" | "warn";
}) {
  const message = error || helper;
  return (
    <label className={`field ${error || tone === "warn" ? "field-warn" : ""}`}>
      <span>{label}</span>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        type={type}
        autoComplete={type === "password" ? "current-password" : "off"}
        aria-invalid={Boolean(error)}
      />
      {message && <small>{message}</small>}
    </label>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
  icon,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  icon: ReactNode;
}) {
  return (
    <button className={`toggle-row ${checked ? "checked" : ""}`} type="button" onClick={() => onChange(!checked)}>
      <span>
        {icon}
        {label}
      </span>
      <span className="toggle-track" aria-hidden="true">
        <span />
      </span>
    </button>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric-row">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function StatusPill({ label, tone }: { label: string; tone: "good" | "warn" | "idle" }) {
  return <span className={`status-pill ${tone}`}>{label}</span>;
}

function StaticPill({ icon, label }: { icon: ReactNode; label: string }) {
  return (
    <div className="static-pill">
      {icon}
      <span>{label}</span>
    </div>
  );
}

export default App;
