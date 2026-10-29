# Monograph 09: Two Ancestors — yuzu, Ryujinx and Their Forks

> [!NOTE]
> **Status: source-checked survey.** Every claim below was checked against either upstream/primary sources or public reference repositories (Eden, Ryujinx 1.1.1403, Ryubing). Claims we could not source were left out. Where the evidence only supports a mechanism, we state the mechanism and no performance figure. HTML comments at the end of each line are internal trace tags (`V` = verification table row, `R` = project fact) used by reviewers; they do not render.

The two best-known Nintendo Switch emulators of the last decade took different roads. This monograph surveys them as software engineering projects, analyzing their architectures, CPU translation approaches, and profile-guided optimization mechanisms.

---

## 1. Two Ancestors, Two Philosophies

| | yuzu | Ryujinx |
|---|---|---|
| Language | C++ <!-- V5.4 --> | C# (.NET) <!-- V5.4, V2.1 --> |
| License | GPLv3 (Eden tree: `LICENSE.txt`) <!-- V5.4 --> | MIT (Ryujinx 1.1.1403 tree: `LICENSE.txt`) <!-- V5.4 --> |
| Started | 2018, by the team behind Citra <!-- V5.5 --> | 09/2017, by gdkchan <!-- V5.5 --> |
| Stopped | 03/2024 <!-- V5.5 --> | 01/10/2024 <!-- V5.5 --> |
| Stated emphasis | Speed and simplification (e.g. the "Hades" shader-compiler overhaul, progress report 07/2021) <!-- V5.1 --> | Accuracy, per the project's own documentation <!-- V5.1 --> |

Both projects ended in 2024; the sequence of events is analyzed in §5 and the sources are listed in §6. <!-- V5.5 -->

The emphasis row describes each project's *stated* direction and how the community has recorded it; it is a difference of philosophy, not a ranking. <!-- V5.1 -->

### 1.1 Continuations (as of 10/2026)

| Lineage | Fork | What we can verify |
|---|---|---|
| yuzu | Suyu | Stopped at v0.0.3 <!-- V5.6 --> |
| yuzu | Sudachi | A yuzu fork; its developer later recommended Eden and stopped development <!-- review 056: androidauthority.com/sudachi-ends-development-3590776 --> |
| yuzu | Citron | Continues the line <!-- V5.6 --> |
| yuzu | Eden | yuzu-derived; started by developers who previously worked on Citron; ships PGO builds (§3) <!-- V1.1; review 056: emulation.gametechwiki.com/index.php/Eden --> |
| Ryujinx | Ryubing | Moved to newer .NET (reference tree targets `net10.0`) and fixed LightningJit <!-- V5.7 --> |
| Ryujinx | Kenji-NX, Classic, Astris (macOS) | Named continuations <!-- V5.7 --> |

---

## 2. How the Guest CPU Is Translated

Both emulators translate guest ARM code with a JIT, and both use an SSA-style intermediate representation (IR). In the Ryujinx tree the IR lives in `ARMeilleure/IntermediateRepresentation/` with SSA construction in `Translation/SsaConstruction.cs`. <!-- V3.1 -->

| Aspect | dynarmic (yuzu / Eden) | ARMeilleure (Ryujinx) |
|---|---|---|
| Fast memory | Direct host-mapped fastmem (`core/arm/dynarmic/arm_dynarmic_64.cpp`) <!-- V3.2 --> | `MemoryManagerMode`: HostMapped / Unsafe / Software (`Ryujinx.Common/Configuration/MemoryManagerMode.cs`) <!-- V3.2 --> |
| Exclusive access | A dedicated `DynarmicExclusiveMonitor` (`core/arm/exclusive_monitor.cpp`) <!-- V3.3 --> | Atomic instructions used directly <!-- V3.3 --> |
| Tiering | — | Three JIT tiers tied to the PPTC cache (§4) <!-- V3.1 --> |

**LightningJit.** Ryujinx also contains a second CPU engine, `LightningJit`, specialised for ARM devices (PR #6057, 01/2024, shipped from 1.1.1117). <!-- V3.4 -->

**NCE (Native Code Execution).** The yuzu line contains an NCE backend (`core/arm/nce/arm_nce.cpp`) that runs ARM64 guest code natively with no JIT; 32-bit titles fall back to dynarmic. <!-- V3.5 --> The idea was pioneered on Android by Skyline; after Skyline stopped, yuzu carried it forward as Project NiCE. <!-- V3.6 -->

---

## 3. PGO: What It Optimizes in an Emulator

**What PGO touches.** Profile-guided optimization improves the *emulator's own C++ binary*: the dynarmic dispatcher, texture and buffer caches, GPU command handling and the Vulkan layer. It does **not** optimize the translated game code, because dynarmic generates that x86 at run time. <!-- R2 -->
The MSVC mechanism is `/GENPROFILE` followed by `/USEPROFILE` (Microsoft Learn); the guest-code JIT is driven from `core/arm/dynarmic/arm_dynarmic_64.cpp`. <!-- V-R2 -->

**Who ships PGO builds.**

- **Eden** distributes PGO packages (WinGet `Utopia.Eden.MinGW-PGO`, a Fedora COPR RPM, and releases v0.0.4, v0.2.0, v0.2.1). <!-- V1.1 -->
  Its Windows PGO build uses Clang/MinGW, while MSVC is the default stable build. <!-- V1.2 -->
  Profiles are collected from earlier runs to find hot spots; no public game script exists. <!-- V1.3 -->
  Release posts quote different speed-ups from one release to the next, and we have not reproduced any of them, so we give no figure here. <!-- V1.4, review 056: figures differ between sources --> 
- **Citron** offers *optional* PGO scripts (`pgo-build.ps1`, `build-citron-linux.sh`) rather than automated CI builds; it uses Clang, collects profiles with `pgo-build.ps1 generate`. <!-- V1.5, V1.6 -->
- **Sudachi:** we found no official PGO build. <!-- V1.7 -->
- **yuzu:** no official PGO release; community members experimented on Steam Deck. <!-- V1.8 -->

**Why Ryujinx forks ship no separate PGO download.** .NET 8 and later enable Dynamic PGO together with tiered compilation by default: tier 0 gathers measurements, tier 1 re-optimizes hot code. <!-- V2.1 -->
Ryujinx project files enable `<TieredPGO>true</TieredPGO>` in `src/Ryujinx/Ryujinx.csproj` (e.g. line 11 in Ryujinx 1.1.1403, line 10 in Ryubing); an older release (1.1.54) had tiered compilation turned off. <!-- V2.2 -->
Official Ryujinx build configurations do not enable Native AOT, and ReadyToRun is not enabled in standard publish configurations (`PublishSingleFile=true`). <!-- V2.3, V2.4 -->
Our reading, not a project statement: because the runtime tunes hot code while the program runs, and PPTC already stores the hot translated ARM code, a static profile download adds little. <!-- V2.5 -->

---

## 4. PPTC, AOT and the 2026 Recompilation Wave

**PPTC.** Ryujinx's persistent translation cache shipped in 1.0.4709 on 16 June 2020, developed by Ricken. <!-- V4.1 -->
It stores the translation info, code, relocation and unwind streams, compressed with Deflate at the Fastest level (`ARMeilleure/Translation/PTC/Ptc.cs`, lines 46–54). <!-- V4.2 -->
It shortens load time and the first-run stutter; it does not raise the steady-state FPS ceiling once code is in RAM. <!-- V4.3, R4 -->
By contrast, we found no on-disk CPU translation cache in the yuzu/Eden CPU tree; what exists is a disk *shader* cache and an in-RAM JIT cache. <!-- V4.4 -->

**Ahead-of-time (AOT) translation.** Compiling the guest code ahead of time and linking it into an emulator did not beat dynarmic's JIT on x86-64 in the measurements we have: mk8-recomp reached 56.1–59.8 FPS against 60 FPS under the JIT, and its first build ran at 23.7 FPS. <!-- R3 -->

**The 2026 recompilation wave.** Today's recompilation projects translate AArch64 to C statically, compile on the player's machine, map NVN directly to Vulkan and use a reduced HLE OS. <!-- R6 -->
Ryujinx code lives on there: its Shader Translator (MIT) is used by several recompilation projects (for example MacNativeGames and AC-Recomp), and LASRecomp ports the audio renderer from Ryujinx. <!-- R5 -->

**Is PPTC the ancestor of recompilation?** The evidence supports only a difference of method. PPTC is dynamic translation whose results are saved and which still runs inside the emulator; recompilation translates everything ahead of time and runs outside the emulator. <!-- R6 -->
Both reuse a translated result instead of retranslating, but we found no source stating that one influenced the other, so we draw no lineage conclusion. <!-- R6, V4.3 -->

---

## 5. How the Two Projects Ended

**yuzu (February–March 2024):**
On 26 February 2024, Nintendo of America filed a complaint against Tropic Haze LLC (the entity operating yuzu) in the U.S. District Court for the District of Rhode Island, alleging violations of the anti-circumvention provisions of the Digital Millennium Copyright Act (DMCA). <!-- V6.1 -->
The complaint alleged that yuzu facilitated piracy on a massive scale, specifically citing that a major Nintendo title was pirated over one million times before its official release, with piracy websites directing users to yuzu. <!-- V6.1 -->
Nintendo highlighted that Tropic Haze monetized yuzu through Patreon, where paying subscribers received "early access" builds containing daily updates and performance improvements. According to the complaint, Patreon memberships doubled in the days the leaked copy circulated before launch; press coverage of the filing put the page at more than 7,000 patrons and about $30,000 per month. <!-- V6.2 -->
On 4 March 2024, the parties reached a stipulated settlement: Tropic Haze agreed to pay $2.4 million in damages, surrender the `yuzu-emu.org` domain, delete circumvention tools, and permanently cease development and distribution of both yuzu and Citra. Tropic Haze stated they always opposed piracy. *These allegations represent the claims set forth in the complaint and were not adjudicated in court due to the settlement.* <!-- V6.3 -->

**Ryujinx (October 2024):**
On 1 October 2024, the Ryujinx project was discontinued after Nintendo contacted its creator and lead developer, gdkchan, offering an agreement to cease development, dismantle the GitHub organization, and remove all project assets. <!-- V6.4 -->
**No lawsuit was filed in court**, and it remains unconfirmed whether the agreement involved financial compensation. <!-- V6.4 -->
Ryujinx's Patreon campaign historically had around 800 patrons generating between $1,500 and $1,700 per month. The shutdown was communicated to the community on Discord by contributor riperiperi. <!-- V6.4, V6.5 -->

**Distribution and funding comparison:**
The two projects operated under contrasting distribution models: yuzu maintained a paywalled "early access" tier on Patreon for preliminary builds, whereas Ryujinx's Patreon offered no early builds: patrons got early progress reports, and every build was public and free. <!-- V6.2, V6.5 -->

**Enduring technical milestones:**
Both codebases produced architectural innovations that persist across the emulation ecosystem:
- **Project Hades (yuzu, 07/2021):** Overhauled shader translation into SSA IR targeting SPIR-V/GLSL/GLASM, resolving long-standing stutter. <!-- V5.2 -->
- **Project NiCE (yuzu, 11/2023):** Native Code Execution for ARM64 Android, building upon pioneering concepts from Skyline. <!-- V3.6 -->
- **PPTC (Ryujinx, 06/2020):** Disk caching of JIT translation streams, dramatically cutting startup latency. <!-- V4.1 -->
- **Recompilation adoption:** Ryujinx's MIT-licensed Shader Translator and Audio Renderer continue to power 2026 static recompilation runtimes. <!-- R5 -->

---

## 6. Sources

**Public and upstream repositories:**

- `Ryujinx/Ryujinx` mirror: `src/Ryujinx/Ryujinx.csproj` (lines 11, 21); `src/ARMeilleure/Translation/PTC/Ptc.cs` (lines 46–54, 497); `src/ARMeilleure/IntermediateRepresentation/`; `src/Ryujinx.Cpu/LightningJit/`; `src/Ryujinx.Common/Configuration/MemoryManagerMode.cs` (7–12)
- `Ryubing`: `src/Ryujinx/Ryujinx.csproj` (line 10); `Directory.Build.props` (line 3)
- `Eden`: `src/core/arm/dynarmic/arm_dynarmic_64.cpp`; `src/core/arm/exclusive_monitor.cpp`; `src/core/arm/nce/arm_nce.cpp`; `LICENSE.txt`

**External sources & reporting:**

- Microsoft Learn, .NET 8 runtime (Dynamic PGO, tiered compilation): https://learn.microsoft.com/en-us/dotnet/core/whats-new/dotnet-8/runtime
- Eden PGO packages: https://winstall.app/apps/Utopia.Eden.MinGW-PGO · https://copr.fedorainfracloud.org/coprs/myriad-sun/eden · https://www.reddit.com/r/EdenEmulator/comments/1r56b1e/eden_v020rc1_out_now_with_several_new_features/ · https://shattered.io/eden-setup-guide/
- Eden lineage: https://emulation.gametechwiki.com/index.php/Eden · Sudachi: https://www.androidauthority.com/sudachi-ends-development-3590776/
- Citron: https://github.com/pkgforge-dev/Anylinux-AppImages/issues/14 · https://newreleases.io/project/github/Zephyron-Dev/Citron-CI/release/2026.02.1 · https://www.reddit.com/r/EmulationOnAndroid/comments/1o3qwfw/citron_v080_released_this_is_a_special/ · https://www.reddit.com/r/EmulationOnAndroid/comments/1oi6hjm/citron_v090_released_this_release_brings/
- yuzu lawsuit & Patreon coverage:
  - GameSpot: https://www.gamespot.com/articles/nintendo-suing-creators-of-switch-emulator-says-tears-of-the-kingdom-was-pirated-1-million-times-pre-launch/1100-6521417/
  - Patreon figures (as reported from the filing): https://overkill.wtf/nintendo-sue-yuzu-emulator · https://www.tweaktown.com/news/96507/nintendo-is-suing-the-creators-of-switch-emulator-yuzu-for-unlawfully-circumventing-its-tech/index.html
  - Settlement coverage: https://apnews.com/article/nintendo-switch-emulator-yuzu-settlement-tropic-haze-49b062ca3897ca2687c711fa7e1d51ee · https://www.aol.com/nintendo-reaches-settlement-ri-based-192601848.html
  - yuzu progress reports archive: https://yuzu-mirror.github.io/entry/yuzu-art/ · PGO experiments: https://github.com/zamazan4ik/awesome-pgo/blob/main/article/article.md
- Ryujinx shutdown & documentation:
  - Wikipedia: https://en.wikipedia.org/wiki/Ryujinx
  - GamingOnLinux: https://www.gamingonlinux.com/2024/10/nintendo-switch-emulator-ryujinx-gets-shut-down/
  - Ryujinx Patreon perks were progress reports and developer interviews, not builds (Ryujinx README, mirror): https://git.devminer.xyz/archive/Ryujinx/commit/7a1ab71c7376bcc3d043c09c9ac1172637955e54
  - Hitmarker: https://hitmarker.net/news/switch-emulator-ryujinx-has-been-taken-down-after-nintendo-contacted-its-lead-developer-939861
  - Documentation archive: https://docs.ryujinx.app/ · 1.1.54 project file: https://git.zajc.tel/mirror/Ryujinx.git/tree/Ryujinx/Ryujinx.csproj?h=1.1.54 · PPTC release coverage: https://wccftech.com/ryujinx-nintendo-switch-emulator-version-1-0-4709-loading-times-70/ · https://gbatemp.net/threads/ryujinx-update-reduces-game-load-times-up-to-70.567655/ · https://yakushabb-mirror-ryujinx.mintlify.app/features/ptc
- dynarmic: https://github.com/azahar-emu/dynarmic/blob/master/README.md
- NCE / Skyline: https://www.androidauthority.com/nintendo-switch-emulation-progress-3567778/
- Ryujinx continuations: https://gitlab.com/ryujinx-classic/ryujinx-classic · https://github.com/Ryubing/LdnServer/blob/master/flake.nix · https://www.reddit.com/r/macgaming/comments/1tur64y/new_nintendo_switch_emulator_for_mac/
