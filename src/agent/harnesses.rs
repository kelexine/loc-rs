//! Registry of AI coding agents / harnesses.
//!
//! All data is resolved at compile time via `const`/`static` definitions.
//! [`EnvPattern`] encodes match semantics in the type system rather than
//! parsing strings at runtime.
//!
//! Detection priority follows insertion order in [`AGENT_HARNESSES`]; the
//! first matching entry wins (e.g. `cowork` must precede `claude-code`).
//!
//! # Author
//! kelexine <https://github.com/kelexine>

// ─── Env-var match pattern ────────────────────────────────────────────────────

/// Compile-time pattern for matching an environment variable's value.
///
/// Mirrors the three-case TS convention:
/// - `"*"`          → [`EnvPattern::Any`]
/// - `"<value>"`    → [`EnvPattern::Exact`]
/// - `"<prefix>*"`  → [`EnvPattern::Prefix`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPattern {
    /// Variable is set to any non-empty value.
    Any,
    /// Variable equals this exact string.
    Exact(&'static str),
    /// Variable value begins with this prefix.
    Prefix(&'static str),
}

impl EnvPattern {
    /// Returns `true` if `value` satisfies the pattern.
    #[inline]
    pub const fn matches(self, value: &str) -> bool {
        match self {
            Self::Any => !value.is_empty(),
            Self::Exact(expected) => const_str_eq(value, expected),
            Self::Prefix(prefix) => const_str_starts_with(value, prefix),
        }
    }
}

// ─── Env-var check ────────────────────────────────────────────────────────────

/// A single (variable-name, pattern) pair used for harness detection.
#[derive(Debug, Clone, Copy)]
pub struct EnvVarCheck {
    /// Name of the environment variable to inspect.
    pub name: &'static str,
    /// Pattern the variable's value must satisfy.
    pub pattern: EnvPattern,
}

// ─── Harness descriptor ───────────────────────────────────────────────────────

/// Descriptor for a known AI agent harness.
///
/// All fields are `&'static str` so the struct is entirely stack-resident and
/// zero-cost to copy — no heap allocation ever occurs.
#[derive(Debug, Clone, Copy)]
pub struct AgentHarness {
    /// Human-readable display name (e.g. used in leaderboards).
    pub pretty_label: &'static str,
    /// URL to the harness's source repository, if public.
    pub repo_url: Option<&'static str>,
    /// URL to the harness's documentation or website.
    pub docs_url: Option<&'static str>,
    /// Short prose description of the harness.
    pub description: Option<&'static str>,
    /// Environment variable checks; detection succeeds on the **first** match.
    /// An empty slice means detection relies solely on [`STANDARD_AGENT_ENV_VARS`].
    pub env_vars: &'static [EnvVarCheck],
}

// ─── Harness key (compile-time enum) ─────────────────────────────────────────

/// Compile-time enumeration of every known agent harness ID.
///
/// Equivalent to `AgentHarnessKey = keyof typeof AGENT_HARNESSES` in
/// TypeScript — but resolved entirely at compile time instead of being a
/// string union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentHarnessKey {
    Antigravity,
    AugmentCli,
    Cline,
    Cowork,
    ClaudeCode,
    Codex,
    Crush,
    GeminiCli,
    GithubCopilot,
    Goose,
    HermesAgent,
    KiloCode,
    Kiro,
    OpenClaw,
    OpenCode,
    Pi,
    Replit,
    Trae,
    Warp,
    Zed,
    CursorCli,
    Cursor,
    Devin,
}

impl AgentHarnessKey {
    /// Canonical string ID for this key (the map key used in the TS source).
    #[inline]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Antigravity   => "antigravity",
            Self::AugmentCli    => "augment-cli",
            Self::Cline         => "cline",
            Self::Cowork        => "cowork",
            Self::ClaudeCode    => "claude-code",
            Self::Codex         => "codex",
            Self::Crush         => "crush",
            Self::GeminiCli     => "gemini-cli",
            Self::GithubCopilot => "github-copilot",
            Self::Goose         => "goose",
            Self::HermesAgent   => "hermes-agent",
            Self::KiloCode      => "kilo-code",
            Self::Kiro          => "kiro",
            Self::OpenClaw      => "openclaw",
            Self::OpenCode      => "opencode",
            Self::Pi            => "pi",
            Self::Replit        => "replit",
            Self::Trae          => "trae",
            Self::Warp          => "warp",
            Self::Zed           => "zed",
            Self::CursorCli     => "cursor-cli",
            Self::Cursor        => "cursor",
            Self::Devin         => "devin",
        }
    }

    /// Parse a string ID into the corresponding key, or `None` if unrecognised.
    ///
    /// This is a `const fn` — the compiler will fold calls with literal
    /// arguments into a compile-time constant.
    #[inline]
    pub const fn from_id(id: &str) -> Option<Self> {
        // `match` on byte slices is the idiomatic const-fn string switch.
        match id.as_bytes() {
            b"antigravity"    => Some(Self::Antigravity),
            b"augment-cli"    => Some(Self::AugmentCli),
            b"cline"          => Some(Self::Cline),
            b"cowork"         => Some(Self::Cowork),
            b"claude-code"    => Some(Self::ClaudeCode),
            b"codex"          => Some(Self::Codex),
            b"crush"          => Some(Self::Crush),
            b"gemini-cli"     => Some(Self::GeminiCli),
            b"github-copilot" => Some(Self::GithubCopilot),
            b"goose"          => Some(Self::Goose),
            b"hermes-agent"   => Some(Self::HermesAgent),
            b"kilo-code"      => Some(Self::KiloCode),
            b"kiro"           => Some(Self::Kiro),
            b"openclaw"       => Some(Self::OpenClaw),
            b"opencode"       => Some(Self::OpenCode),
            b"pi"             => Some(Self::Pi),
            b"replit"         => Some(Self::Replit),
            b"trae"           => Some(Self::Trae),
            b"warp"           => Some(Self::Warp),
            b"zed"            => Some(Self::Zed),
            b"cursor-cli"     => Some(Self::CursorCli),
            b"cursor"         => Some(Self::Cursor),
            b"devin"          => Some(Self::Devin),
            _                 => None,
        }
    }

    /// Returns the static [`AgentHarness`] descriptor for this key.
    #[inline]
    pub const fn info(self) -> &'static AgentHarness {
        // Linear scan of a 23-element array; the compiler constant-folds this
        // when `self` is known at compile time.
        let mut i = 0;
        while i < AGENT_HARNESSES.len() {
            if AGENT_HARNESSES[i].0 as u8 == self as u8 {
                return &AGENT_HARNESSES[i].1;
            }
            i += 1;
        }
        // INVARIANT: every AgentHarnessKey variant must be present in AGENT_HARNESSES.
        panic!("AgentHarnessKey variant missing from AGENT_HARNESSES");
    }

    /// All known harness keys, in detection-priority order.
    pub const ALL: &'static [AgentHarnessKey] = &[
        Self::Antigravity,
        Self::AugmentCli,
        Self::Cline,
        Self::Cowork,
        Self::ClaudeCode,
        Self::Codex,
        Self::Crush,
        Self::GeminiCli,
        Self::GithubCopilot,
        Self::Goose,
        Self::HermesAgent,
        Self::KiloCode,
        Self::Kiro,
        Self::OpenClaw,
        Self::OpenCode,
        Self::Pi,
        Self::Replit,
        Self::Trae,
        Self::Warp,
        Self::Zed,
        Self::CursorCli,
        Self::Cursor,
        Self::Devin,
    ];
}

impl core::fmt::Display for AgentHarnessKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.id())
    }
}

// ─── Standard agent env-vars ──────────────────────────────────────────────────

/// Standard environment variables that any tool can set to identify itself.
///
/// When one of these is set, its value is treated directly as an
/// [`AgentHarnessKey`] ID; unrecognised values are reported as `"unknown"`.
pub const STANDARD_AGENT_ENV_VARS: &[&str] = &["AI_AGENT", "AGENT"];

// ─── Registry ─────────────────────────────────────────────────────────────────

/// Ordered registry of all known agent harnesses.
///
/// **Insertion order determines detection priority**: harnesses are checked
/// top-to-bottom and the first match wins.  In particular, `Cowork` must
/// remain before `ClaudeCode` so the more-specific `CLAUDE_CODE_IS_COWORK`
/// signal takes priority when both variables are set.
///
/// Each entry is `(AgentHarnessKey, AgentHarness)`.  The entire array lives
/// in the binary's read-only data segment — zero heap allocation.
pub const AGENT_HARNESSES: &[(AgentHarnessKey, AgentHarness)] = &[
    (
        AgentHarnessKey::Antigravity,
        AgentHarness {
            pretty_label: "Antigravity",
            repo_url:     None,
            docs_url:     Some("https://antigravity.google"),
            description:  Some("Agentic development platform from Google built around Gemini."),
            env_vars: &[EnvVarCheck { name: "ANTIGRAVITY_AGENT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::AugmentCli,
        AgentHarness {
            pretty_label: "Augment CLI",
            repo_url:     Some("https://github.com/augmentcode/auggie"),
            docs_url:     Some("https://www.augmentcode.com"),
            description:  Some("Auggie, the command-line coding agent from Augment Code."),
            env_vars: &[EnvVarCheck { name: "AUGMENT_AGENT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Cline,
        AgentHarness {
            pretty_label: "Cline",
            repo_url:     Some("https://github.com/cline/cline"),
            docs_url:     Some("https://cline.bot"),
            description:  Some("Open-source autonomous coding agent for VS Code."),
            env_vars: &[EnvVarCheck { name: "CLINE_ACTIVE", pattern: EnvPattern::Any }],
        },
    ),
    (
        // Must stay before `ClaudeCode`: when both `CLAUDE_CODE` and
        // `CLAUDE_CODE_IS_COWORK` are set, the more-specific Cowork signal wins.
        AgentHarnessKey::Cowork,
        AgentHarness {
            pretty_label: "Cowork",
            repo_url:     None,
            docs_url:     Some("https://claude.com/product/cowork"),
            description:  Some("Anthropic's agent for autonomous knowledge work, built on top of Claude Code."),
            env_vars: &[EnvVarCheck { name: "CLAUDE_CODE_IS_COWORK", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::ClaudeCode,
        AgentHarness {
            pretty_label: "Claude Code",
            repo_url:     Some("https://github.com/anthropics/claude-code"),
            docs_url:     Some("https://code.claude.com/docs"),
            description:  Some("Anthropic's agentic coding tool that lives in your terminal."),
            env_vars: &[
                EnvVarCheck { name: "CLAUDECODE",  pattern: EnvPattern::Any },
                EnvVarCheck { name: "CLAUDE_CODE", pattern: EnvPattern::Any },
            ],
        },
    ),
    (
        AgentHarnessKey::Codex,
        AgentHarness {
            pretty_label: "Codex",
            repo_url:     Some("https://github.com/openai/codex"),
            docs_url:     Some("https://developers.openai.com/codex"),
            description:  Some("OpenAI's lightweight coding agent that runs in your terminal."),
            env_vars: &[
                EnvVarCheck { name: "CODEX_SANDBOX",   pattern: EnvPattern::Any },
                EnvVarCheck { name: "CODEX_CI",        pattern: EnvPattern::Any },
                EnvVarCheck { name: "CODEX_THREAD_ID", pattern: EnvPattern::Any },
            ],
        },
    ),
    (
        AgentHarnessKey::Crush,
        AgentHarness {
            pretty_label: "Crush",
            repo_url:     Some("https://github.com/charmbracelet/crush"),
            docs_url:     Some("https://github.com/charmbracelet/crush"),
            description:  Some("Charm's open-source AI coding agent for the terminal."),
            env_vars: &[EnvVarCheck { name: "CRUSH", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::GeminiCli,
        AgentHarness {
            pretty_label: "Gemini CLI",
            repo_url:     Some("https://github.com/google-gemini/gemini-cli"),
            docs_url:     Some("https://geminicli.com"),
            description:  Some("Google's open-source terminal AI coding agent powered by Gemini models."),
            env_vars: &[EnvVarCheck { name: "GEMINI_CLI", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::GithubCopilot,
        AgentHarness {
            pretty_label: "GitHub Copilot",
            repo_url:     None,
            docs_url:     Some("https://docs.github.com/copilot"),
            description:  Some("GitHub's AI coding assistant."),
            env_vars: &[
                EnvVarCheck { name: "COPILOT_MODEL",       pattern: EnvPattern::Any },
                EnvVarCheck { name: "COPILOT_ALLOW_ALL",   pattern: EnvPattern::Any },
                EnvVarCheck { name: "COPILOT_GITHUB_TOKEN",pattern: EnvPattern::Any },
            ],
        },
    ),
    (
        AgentHarnessKey::Goose,
        AgentHarness {
            pretty_label: "Goose",
            repo_url:     Some("https://github.com/aaif-goose/goose"),
            docs_url:     Some("https://goose-docs.ai/"),
            description:  Some("Open-source, extensible AI agent, originally from Block and now part of the Agentic AI Foundation."),
            env_vars: &[EnvVarCheck { name: "GOOSE_TERMINAL", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::HermesAgent,
        AgentHarness {
            pretty_label: "Hermes Agent",
            repo_url:     Some("https://github.com/NousResearch/hermes-agent"),
            docs_url:     Some("https://hermes-agent.nousresearch.com/docs"),
            description:  Some("Nous Research's self-improving, multi-provider terminal AI agent."),
            env_vars: &[EnvVarCheck { name: "HERMES_SESSION_ID", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::KiloCode,
        AgentHarness {
            pretty_label: "Kilo Code",
            repo_url:     Some("https://github.com/Kilo-Org/kilocode"),
            docs_url:     Some("https://kilocode.ai/docs"),
            description:  Some("Open-source agentic coding agent for VS Code, JetBrains, and the terminal."),
            env_vars: &[EnvVarCheck { name: "KILOCODE_FEATURE", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Kiro,
        AgentHarness {
            pretty_label: "Kiro",
            repo_url:     None,
            docs_url:     Some("https://kiro.dev"),
            description:  Some("AWS's agentic IDE for spec-driven AI software development."),
            env_vars: &[EnvVarCheck { name: "AGENT_CONTEXT_OUT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::OpenClaw,
        AgentHarness {
            pretty_label: "OpenClaw",
            repo_url:     Some("https://github.com/openclaw/openclaw"),
            docs_url:     Some("https://openclaw.ai"),
            description:  Some("Open-source, self-hosted personal AI assistant that runs on your own devices."),
            env_vars: &[EnvVarCheck { name: "OPENCLAW_SHELL", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::OpenCode,
        AgentHarness {
            pretty_label: "opencode",
            repo_url:     Some("https://github.com/anomalyco/opencode"),
            docs_url:     Some("https://opencode.ai"),
            description:  Some("Open-source AI coding agent built for the terminal."),
            env_vars: &[EnvVarCheck { name: "OPENCODE_CLIENT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Pi,
        AgentHarness {
            pretty_label: "Pi",
            repo_url:     Some("https://github.com/earendil-works/pi"),
            docs_url:     Some("https://pi.dev"),
            description:  Some("Minimal, self-extensible terminal coding agent with a unified multi-provider LLM API."),
            env_vars: &[EnvVarCheck { name: "PI_CODING_AGENT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Replit,
        AgentHarness {
            pretty_label: "Replit",
            repo_url:     None,
            docs_url:     Some("https://replit.com"),
            description:  Some("Cloud development environment with an AI coding agent."),
            env_vars: &[EnvVarCheck { name: "REPL_ID", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Trae,
        AgentHarness {
            pretty_label: "Trae",
            repo_url:     None,
            docs_url:     Some("https://trae.ai"),
            description:  Some("AI-powered IDE from ByteDance."),
            env_vars: &[EnvVarCheck { name: "TRAE_AI_SHELL_ID", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Warp,
        AgentHarness {
            pretty_label: "Warp",
            repo_url:     Some("https://github.com/warpdotdev/Warp"),
            docs_url:     Some("https://docs.warp.dev"),
            description:  Some("AI-powered terminal with an agentic Agent Mode."),
            // Exact match: TERM_PROGRAM == "WarpTerminal"
            env_vars: &[EnvVarCheck { name: "TERM_PROGRAM", pattern: EnvPattern::Exact("WarpTerminal") }],
        },
    ),
    (
        AgentHarnessKey::Zed,
        AgentHarness {
            pretty_label: "Zed",
            repo_url:     Some("https://github.com/zed-industries/zed"),
            docs_url:     Some("https://zed.dev"),
            description:  Some("High-performance code editor with an integrated AI agent panel and terminal."),
            env_vars: &[EnvVarCheck { name: "ZED_TERM", pattern: EnvPattern::Any }],
        },
    ),
    (
        // Kept before `Cursor`: child processes inside Cursor's terminal
        // inherit CURSOR_TRACE_ID, so `cursor` must be a lower-priority
        // fallback. `cursor-cli` (CURSOR_AGENT) is the more specific signal.
        AgentHarnessKey::CursorCli,
        AgentHarness {
            pretty_label: "Cursor CLI",
            repo_url:     None,
            docs_url:     Some("https://cursor.com/docs/cli/overview"),
            description:  Some("Cursor's coding agent for the command line."),
            env_vars: &[EnvVarCheck { name: "CURSOR_AGENT", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Cursor,
        AgentHarness {
            pretty_label: "Cursor",
            repo_url:     None,
            docs_url:     Some("https://cursor.com"),
            description:  Some("AI-powered code editor."),
            env_vars: &[EnvVarCheck { name: "CURSOR_TRACE_ID", pattern: EnvPattern::Any }],
        },
    ),
    (
        AgentHarnessKey::Devin,
        AgentHarness {
            pretty_label: "Devin",
            repo_url:     None,
            docs_url:     Some("https://devin.ai"),
            description:  Some("Autonomous AI software engineer from Cognition."),
            // No specific env-var; detected only via STANDARD_AGENT_ENV_VARS.
            env_vars: &[],
        },
    ),
];

// ─── Compile-time helper fns ──────────────────────────────────────────────────

/// Byte-by-byte string equality usable inside `const fn`.
const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Prefix check usable inside `const fn`.
const fn const_str_starts_with(s: &str, prefix: &str) -> bool {
    let s = s.as_bytes();
    let p = prefix.as_bytes();
    if s.len() < p.len() {
        return false;
    }
    let mut i = 0;
    while i < p.len() {
        if s[i] != p[i] {
            return false;
        }
        i += 1;
    }
    true
}

// ─── Detection ────────────────────────────────────────────────────────────────

/// Result of an agent-harness detection attempt.
///
/// Mirrors the TS behaviour described in [`STANDARD_AGENT_ENV_VARS`]:
/// - A recognised value in a standard env-var → [`DetectionResult::Known`]
/// - A set but unrecognised value in a standard env-var → [`DetectionResult::Unknown`]
/// - A harness-specific env-var match → [`DetectionResult::Known`]
/// - Nothing matched → [`DetectionResult::None`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionResult {
    /// A known harness was identified.
    Known(AgentHarnessKey),
    /// `AI_AGENT` or `AGENT` was set but its value isn't in the registry.
    /// The raw value is preserved so the caller can report it as `"unknown"`.
    Unknown(String),
    /// No harness could be detected from the current environment.
    None,
}

impl DetectionResult {
    /// Returns the detected key, or `None` if the result is `Unknown` or `None`.
    #[inline]
    pub fn key(&self) -> Option<AgentHarnessKey> {
        match self {
            Self::Known(k) => Some(*k),
            _ => Option::None,
        }
    }

    /// Returns `true` if any signal was detected (known **or** unknown).
    #[inline]
    pub fn is_agent(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl core::fmt::Display for DetectionResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Known(k)   => write!(f, "{}", k.id()),
            Self::Unknown(v) => write!(f, "unknown ({})", v),
            Self::None       => f.write_str("none"),
        }
    }
}

/// Detects the active agent harness from the current process environment.
///
/// # Detection order
///
/// 1. **Harness-specific vars** (primary): [`AGENT_HARNESSES`] is scanned in
///    insertion order (highest priority first).  For each harness, every
///    [`EnvVarCheck`] is tested; the first harness whose **any** check passes
///    is returned as [`DetectionResult::Known`].  A dedicated marker variable
///    is a stronger, more intentional signal than a generic `AI_AGENT` value.
///
/// 2. **Standard vars** (fallback): if no harness-specific var matched, each
///    var in [`STANDARD_AGENT_ENV_VARS`] (`AI_AGENT`, `AGENT`) is checked.
///    Its value is used directly as a harness ID: a recognised value returns
///    [`DetectionResult::Known`]; an unrecognised one returns
///    [`DetectionResult::Unknown`].  Handles harnesses like `devin` that
///    carry no dedicated env-var, and lets a caller force an ID explicitly.
///
/// 3. If nothing matched, returns [`DetectionResult::None`].
///
/// The registry data consulted is entirely compile-time (`const`); only the
/// `std::env::var` calls happen at runtime.
pub fn detect() -> DetectionResult {
    // Phase 1 — harness-specific env-vars (insertion order = priority order).
    // A dedicated marker is a stronger signal than a generic AI_AGENT value.
    for (key, harness) in AGENT_HARNESSES {
        for check in harness.env_vars {
            if let Ok(val) = std::env::var(check.name) {
                if check.pattern.matches(&val) {
                    return DetectionResult::Known(*key);
                }
            }
        }
    }

    // Phase 2 — standard agent env-vars (fallback / explicit override).
    for &var in STANDARD_AGENT_ENV_VARS {
        if let Ok(val) = std::env::var(var) {
            if val.is_empty() {
                continue;
            }
            return match AgentHarnessKey::from_id(&val) {
                Some(key) => DetectionResult::Known(key),
                None      => DetectionResult::Unknown(val),
            };
        }
    }

    DetectionResult::None
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── EnvPattern::matches ──────────────────────────────────────────────────

    #[test]
    fn env_pattern_any_matches_nonempty() {
        assert!(EnvPattern::Any.matches("1"));
        assert!(EnvPattern::Any.matches("true"));
    }

    #[test]
    fn env_pattern_any_rejects_empty() {
        assert!(!EnvPattern::Any.matches(""));
    }

    #[test]
    fn env_pattern_exact_matches() {
        assert!(EnvPattern::Exact("WarpTerminal").matches("WarpTerminal"));
    }

    #[test]
    fn env_pattern_exact_rejects_mismatch() {
        assert!(!EnvPattern::Exact("WarpTerminal").matches("xterm"));
        assert!(!EnvPattern::Exact("WarpTerminal").matches(""));
    }

    #[test]
    fn env_pattern_prefix_matches() {
        assert!(EnvPattern::Prefix("v1.").matches("v1.2.3"));
        assert!(EnvPattern::Prefix("v1.").matches("v1."));
    }

    #[test]
    fn env_pattern_prefix_rejects_too_short() {
        assert!(!EnvPattern::Prefix("v1.").matches("v1"));
        assert!(!EnvPattern::Prefix("v1.").matches(""));
    }

    // ── AgentHarnessKey::from_id ─────────────────────────────────────────────

    #[test]
    fn from_id_round_trips_all_keys() {
        for &key in AgentHarnessKey::ALL {
            assert_eq!(AgentHarnessKey::from_id(key.id()), Some(key));
        }
    }

    #[test]
    fn from_id_returns_none_for_unknown() {
        assert_eq!(AgentHarnessKey::from_id("unknown-agent"), None);
        assert_eq!(AgentHarnessKey::from_id(""), None);
    }

    // ── AGENT_HARNESSES coverage ─────────────────────────────────────────────

    #[test]
    fn all_keys_have_entry_in_registry() {
        for &key in AgentHarnessKey::ALL {
            // info() panics if a key is missing — so this doubles as a coverage check.
            let _ = key.info();
        }
    }

    #[test]
    fn registry_length_matches_all_array() {
        assert_eq!(AGENT_HARNESSES.len(), AgentHarnessKey::ALL.len());
    }

    #[test]
    fn pretty_labels_are_nonempty() {
        for &key in AgentHarnessKey::ALL {
            assert!(
                !key.info().pretty_label.is_empty(),
                "{key} has empty pretty_label",
            );
        }
    }

    #[test]
    fn cowork_precedes_claude_code_in_registry() {
        let pos = |target: AgentHarnessKey| {
            AGENT_HARNESSES
                .iter()
                .position(|(k, _)| *k == target)
                .unwrap()
        };
        assert!(
            pos(AgentHarnessKey::Cowork) < pos(AgentHarnessKey::ClaudeCode),
            "Cowork must appear before ClaudeCode in AGENT_HARNESSES",
        );
    }

    #[test]
    fn cursor_cli_precedes_cursor_in_registry() {
        let pos = |target: AgentHarnessKey| {
            AGENT_HARNESSES
                .iter()
                .position(|(k, _)| *k == target)
                .unwrap()
        };
        assert!(
            pos(AgentHarnessKey::CursorCli) < pos(AgentHarnessKey::Cursor),
            "CursorCli must appear before Cursor in AGENT_HARNESSES",
        );
    }

    #[test]
    fn warp_uses_exact_match() {
        let info = AgentHarnessKey::Warp.info();
        assert_eq!(info.env_vars.len(), 1);
        assert_eq!(
            info.env_vars[0].pattern,
            EnvPattern::Exact("WarpTerminal"),
        );
    }

    #[test]
    fn devin_has_no_env_vars() {
        assert!(AgentHarnessKey::Devin.info().env_vars.is_empty());
    }

    // ── const-eval smoke test ────────────────────────────────────────────────

    #[test]
    fn key_id_is_const_evaluable() {
        // The compiler folds this to a compile-time constant.
        const CLAUDE_ID: &str = AgentHarnessKey::ClaudeCode.id();
        assert_eq!(CLAUDE_ID, "claude-code");
    }

    #[test]
    fn from_id_is_const_evaluable() {
        const KEY: Option<AgentHarnessKey> = AgentHarnessKey::from_id("cowork");
        assert_eq!(KEY, Some(AgentHarnessKey::Cowork));
    }

    #[test]
    fn env_pattern_matches_is_const_evaluable() {
        const MATCHES: bool = EnvPattern::Exact("WarpTerminal").matches("WarpTerminal");
        assert!(MATCHES);
    }

    // ── DetectionResult helpers ──────────────────────────────────────────────

    #[test]
    fn detection_result_key_known() {
        let r = DetectionResult::Known(AgentHarnessKey::Codex);
        assert_eq!(r.key(), Some(AgentHarnessKey::Codex));
        assert!(r.is_agent());
    }

    #[test]
    fn detection_result_key_unknown() {
        let r = DetectionResult::Unknown("my-custom-agent".into());
        assert_eq!(r.key(), None);
        assert!(r.is_agent());
    }

    #[test]
    fn detection_result_none() {
        let r = DetectionResult::None;
        assert_eq!(r.key(), None);
        assert!(!r.is_agent());
    }

    #[test]
    fn detection_result_display() {
        assert_eq!(DetectionResult::Known(AgentHarnessKey::Warp).to_string(), "warp");
        assert_eq!(DetectionResult::Unknown("foo".into()).to_string(), "unknown (foo)");
        assert_eq!(DetectionResult::None.to_string(), "none");
    }

    // ── detect() via env-var injection ──────────────────────────────────────
    // Each test uses a unique variable already present in AGENT_HARNESSES so
    // we don't need to mutate shared state — we set the var, run detect(),
    // then immediately remove it. Tests are NOT marked `#[ignore]` because
    // they rely on variables that are absent by default; they do mutate the
    // process environment, so run with `-- --test-threads=1` if parallelism
    // causes flakiness.

    #[test]
    fn detect_standard_var_known() {
        // Standard var resolves to a known key only when no harness-specific
        // var for that harness is set (Phase 1 finds nothing, Phase 2 fires).
        unsafe { std::env::set_var("AI_AGENT", "claude-code") };
        let result = detect();
        unsafe { std::env::remove_var("AI_AGENT") };
        assert_eq!(result, DetectionResult::Known(AgentHarnessKey::ClaudeCode));
    }

    #[test]
    fn detect_standard_var_unknown() {
        unsafe { std::env::set_var("AI_AGENT", "my-obscure-agent") };
        let result = detect();
        unsafe { std::env::remove_var("AI_AGENT") };
        assert_eq!(result, DetectionResult::Unknown("my-obscure-agent".into()));
    }

    #[test]
    fn detect_harness_specific_var() {
        unsafe { std::env::set_var("CRUSH", "1") };
        let result = detect();
        unsafe { std::env::remove_var("CRUSH") };
        assert_eq!(result, DetectionResult::Known(AgentHarnessKey::Crush));
    }

    #[test]
    fn detect_exact_pattern_warp() {
        unsafe { std::env::set_var("TERM_PROGRAM", "WarpTerminal") };
        let result = detect();
        unsafe { std::env::remove_var("TERM_PROGRAM") };
        assert_eq!(result, DetectionResult::Known(AgentHarnessKey::Warp));
    }

    #[test]
    fn detect_exact_pattern_non_warp_terminal_does_not_match() {
        unsafe { std::env::set_var("TERM_PROGRAM", "iTerm.app") };
        let result = detect();
        unsafe { std::env::remove_var("TERM_PROGRAM") };
        assert_ne!(result, DetectionResult::Known(AgentHarnessKey::Warp));
    }

    #[test]
    fn detect_cowork_wins_over_claude_code_when_both_set() {
        unsafe {
            std::env::set_var("CLAUDE_CODE_IS_COWORK", "1");
            std::env::set_var("CLAUDE_CODE", "1");
        }
        let result = detect();
        unsafe {
            std::env::remove_var("CLAUDE_CODE_IS_COWORK");
            std::env::remove_var("CLAUDE_CODE");
        }
        assert_eq!(result, DetectionResult::Known(AgentHarnessKey::Cowork));
    }

    /// Regression: harness-specific var must beat a conflicting standard-var
    /// value.  `AI_AGENT=codex` + `CRUSH=1` must return `Crush`, not `Codex`.
    /// The old (incorrect) implementation returned `Codex` because it checked
    /// standard vars first.
    #[test]
    fn detect_harness_specific_beats_standard_var_conflict() {
        unsafe {
            std::env::set_var("AI_AGENT", "codex");
            std::env::set_var("CRUSH", "1");
        }
        let result = detect();
        unsafe {
            std::env::remove_var("AI_AGENT");
            std::env::remove_var("CRUSH");
        }
        assert_eq!(result, DetectionResult::Known(AgentHarnessKey::Crush));
    }

    #[test]
    fn detect_returns_none_when_no_vars_set() {
        let ai = std::env::var("AI_AGENT").ok();
        let ag = std::env::var("AGENT").ok();
        unsafe {
            std::env::remove_var("AI_AGENT");
            std::env::remove_var("AGENT");
        }
        let any_harness_var_set = AGENT_HARNESSES.iter().any(|(_, h)| {
            h.env_vars.iter().any(|c| std::env::var(c.name).is_ok())
        });
        if !any_harness_var_set {
            assert_eq!(detect(), DetectionResult::None);
        }
        if let Some(v) = ai { unsafe { std::env::set_var("AI_AGENT", v) } }
        if let Some(v) = ag { unsafe { std::env::set_var("AGENT", v) } }
    }
}
