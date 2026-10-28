# Interface languages

Each device can choose Follow system, English, Simplified Chinese, Traditional
Chinese, Japanese, Korean, French, German, Spanish or Turkish. Desktop and phone
settings hold the choice; the terminal command palette cycles its own saved
preference. Regional variants use their language's catalog. Unsupported system
languages use English. The saved `zh` identifier continues to mean Simplified
Chinese.

The canonical UTF-8 JSON catalogs live in
`apps/ios/Kit/Sources/CodyncKit/Resources/Languages/`. iOS bundles them, desktop
imports them, the terminal embeds them, and Android includes that directory as
an asset source. Do not create a second Android copy.

English source phrases are the lookup keys. All catalogs must have identical
keys and preserve every `{placeholder}`. Missing phrases fall back to English;
Traditional Chinese retains its existing Simplified fallback. Translate an
interface template before inserting a user's name or text. Conversation content
is not translated. The desktop catalog test checks key and placeholder parity.

The five additional catalogs began as machine translations of public interface
copy, with reviewed overrides for common controls. They have not had a complete
native-speaker review. The Turkish catalog was translated later, following the
wording below. Translation improvements should change the canonical catalog and
run the catalog test, together with the affected client checks.

For a new language, add its catalog and register its identifier/native name in
Swift `AppLanguage`, desktop `appLanguages`, Android `AppLanguage`, and terminal
`AppLanguage`. Add it to the Swift package resources, desktop imports, and Rust
embedded catalogs; Android picks up the shared JSON automatically. Test explicit
selection, persistence, regional system tags, unknown-language fallback, widgets,
and the desktop tray. Update the iPhone release notes.

## Turkish wording

Turkish addresses the reader with informal "sen" ("Bu sohbeti silmek istediğine emin misin?").
Glossary:

| English | Turkish |
|---|---|
| Bot / Group | Bot / Grup |
| Section | Bölüm |
| Computer | Bilgisayar |
| Host | Host |
| Agent | Ajan |
| Conversation / Thread | Sohbet |
| Session | Oturum |
| Reply / Mention | Yanıt / Anma |
| Steps / Tools | Adımlar / Araçlar |
| Approve / Deny | Onayla / Reddet |
| Routine / Trigger | Rutin / Tetikleyici |
| Connector / Skill | Bağlayıcı / Beceri |
| Extensions | Uzantılar |
| Marketplace | Mağaza |
| Memory / Usage | Hafıza / Kullanım |
| Remote screen / Voice call | Uzak ekran / Sesli arama |
| Pin / Unpin | Sabitle / Sabitlemeyi kaldır |
| Move to section | Bölüme taşı |
| Pair iPhone | iPhone eşle (with a name: "iPhone'u {name} ile eşle") |
| Settings | Ayarlar |
| Delete / Hide | Sil / Gizle |
| Follow system | Sistemi izle |

Kept as they are: Codync, Claude, Codex, iPhone, Webhook, MCP, model names, keyboard shortcuts,
commands and code in backticks. Short labels stay short (Turkish runs about 30% longer).
