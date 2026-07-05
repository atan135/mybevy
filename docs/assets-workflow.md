# MyBevy 项目资源使用方式

## 1. 文档目标

这份文档约定 MyBevy 中所有运行时资源的使用方式，包括图片、图集、字体、音频、3D 模型、UI 配置、i18n 文案、主题文件，以及后续下载内容。

当前项目约定：

- Rust/Bevy 工程根目录是 `project/`
- 首包资源目录是 `project/assets/`
- 当前 Bevy 版本是 `bevy = "0.18.1"`
- 桌面端通过 `project/src/lib.rs` 中的 `AssetPlugin.file_path` 把资源根目录指向 `project/assets`
- Android Gradle 壳工程会把 `project/assets` 打包进 APK assets
- 后续下载资源不放入 `project/assets`，应发布到内容目录或 CDN，并下载到应用私有缓存

因此首包资源代码路径从 `project/assets/` 下面开始写。例如实际文件是：

```text
project/assets/ui/fonts/MyBevyUiCjk-Regular.otf
```

代码里写：

```rust
"ui/fonts/MyBevyUiCjk-Regular.otf"
```

不要写成 `project/assets/ui/fonts/MyBevyUiCjk-Regular.otf`。

这条规则只适用于首包资源。后续下载资源使用独立资源源，例如：

```text
content_cache://2026.06.09.1/models/props/crate/crate.glb
```

场景清单、游戏层场景目录表和 layout 数据也是首包资源时，同样从 `project/assets/` 下一级开始写。例如实际文件是：

```text
project/assets/game/scenes.csv
project/assets/scenes/sample_dungeon_room/scene.ron
project/assets/scenes/sample_dungeon_room/layout.ron
```

注册到场景框架时写：

```rust
"scenes/sample_dungeon_room/scene.ron"
```

不要写成 `project/assets/scenes/sample_dungeon_room/scene.ron`。当前场景框架只实现首包 RON 场景清单加载；后续下载场景清单和 `content_cache://...` 场景加载仍是后续目标。

## 2. 资源来源分类

MyBevy 资源按来源分三类：

- 开发期资源：开发机本地文件，支持快速替换、热加载或环境变量覆盖。
- APK 包内资源：随 `project/assets` 进入 APK assets，离线可用，只读，不可在运行时修改。
- 后续下载资源：从内容服务器或 CDN 下载到应用私有缓存，用清单、版本和哈希管理。

使用原则：

- 启动必需、错误占位、首屏必需、基础 UI 字体和默认配置进首包。
- 大体积模型、场景、皮肤、活动图、可替换 UI 皮肤、可替换图集、远端关卡和运营配置走后续下载。
- 不想进 APK 的资源不要放入 `project/assets`，因为当前 Gradle 会把整个目录映射进 APK assets。

## 3. 目录约定

首包资源目录建议：

```text
project/assets/
|-- audio/
|   |-- ambience/
|   |-- battle/
|   |-- common/
|   |-- music/
|   |-- spatial/
|   |-- ui/
|   `-- voice/
|-- game/
|   `-- scenes.csv
|-- images/
|-- licenses/
|-- models/
|   |-- characters/
|   |-- props/
|   `-- scenes/
|-- scenes/
|   |-- boot/
|   |-- fallback/
|   |-- sample_dungeon_room/
|   `-- touch_ripple/
|-- textures/
|   |-- atlas/
|   `-- single/
|-- ui/
|   |-- fonts/
|   |-- i18n/
|   |-- images/
|   |-- themes/
|   `-- atlas/
`-- android-res/
```

后续内容发布目录建议：

```text
content_dist/
|-- manifest.json
`-- 2026.06.09.1/
    |-- images/
    |-- models/
    |-- scenes/
    |-- textures/
    |-- ui/
    |   |-- i18n/
    |   |-- themes/
    |   `-- atlas/
    `-- audio/
```

本地缓存目录建议：

```text
<app_private_cache>/mybevy-content/
`-- 2026.06.09.1/
    |-- images/
    |-- models/
    |-- textures/
    |-- ui/
    `-- audio/
```

命名规则：

- 文件和目录使用小写英文、数字、下划线或短横线。
- 路径大小写必须和代码完全一致，Android 上尤其要注意。
- 一个复杂资源一个目录，方便放主文件、依赖文件、说明和后续 LOD。
- 源工程文件如 `.blend`、`.psd`、`.kra`、`.spp` 不放运行包；如需入库，放 `source/` 或专门资产仓库。

### 3.1 Git LFS 提交约定

仓库使用 Git LFS 管理 `project/assets/` 下的二进制资源。当前 `.gitattributes` 会让图片、字体、音频、二进制模型和常见源工程文件走 LFS，例如：

- 图片：`.png`、`.jpg`、`.jpeg`、`.webp`、`.avif`、`.bmp`、`.tga`、`.dds`、`.ktx2`
- 字体：`.otf`、`.ttf`、`.woff`、`.woff2`
- 音频：`.ogg`、`.wav`、`.mp3`、`.flac`、`.aac`、`.m4a`
- 模型和源工程：`.glb`、`.bin`、`.fbx`、`.blend`、`.psd`、`.kra`、`.spp`

RON、JSON、TOML、TXT、授权说明、主题和 i18n 文案等可读文本资源保持普通 Git 提交，方便审阅 diff。

当前已经落地的样板场景首包资源：

```text
project/assets/game/scenes.csv
project/assets/scenes/sample_dungeon_room/scene.ron
project/assets/scenes/sample_dungeon_room/layout.ron
project/assets/models/scenes/kaykit_dungeon_remastered/
project/assets/models/props/kaykit_dungeon_remastered/
project/assets/licenses/kaykit_dungeon_remastered_license.txt
project/assets/licenses/kaykit_adventurers_license.txt
```

其中 `game/scenes.csv` 是游戏层场景目录表，`scenes/sample_dungeon_room/scene.ron` 是 framework manifest，`scenes/sample_dungeon_room/layout.ron` 是 game layer prefab/light 摆放数据。CSV、manifest 和 layout 都是文本资源，保持普通 Git 提交；`.gltf`、`.bin`、`.glb`、`.png` 等模型和贴图资源走 Git LFS。

方圆 Bake 产物策略：

- 开发源文件继续使用 RON，放在 `project/assets/fangyuan/` 下时保持普通 Git 提交。
- 默认 dry-run report 和临时输出写入仓库根目录 `artifacts/fangyuan-bake/`，该目录被 Git 忽略，不提交。
- 本地或构建流水线生成的 `.fyb`、`*.bake-report.txt` 和 `fangyuan-bake-report.txt` 默认视为构建产物，已通过 `.gitignore` 忽略。
- 如果后续明确要把某个 `.fyb` 作为首包发布资源放入 `project/assets/`，应进入 Git LFS；当前 `.gitattributes` 已覆盖 `project/assets/**/*.fyb`。
- bake report 只作为审查和 CI 日志产物，不进 LFS，也不随源码提交；需要长期保存时应归档到构建系统 artifact。

首次克隆或换新机器开发时，先确认 Git LFS 可用：

```powershell
git lfs version
git lfs install
```

新增首包二进制资源时，正常放入 `project/assets/` 后执行：

```powershell
git status
git check-attr filter -- project/assets/ui/images/example.png
```

如果输出里有 `filter: lfs`，说明该资源会按 LFS 指针提交。若新增了未覆盖的新二进制格式，应同步更新仓库根目录的 `.gitattributes` 和本文档。

## 4. 开发期使用

### 4.1 桌面运行

所有 Rust 和 Bevy 命令默认在 `project/` 下执行：

```powershell
Set-Location project
cargo run
```

桌面端当前已把 `AssetPlugin.file_path` 固定到 `project/assets`，因此即使直接启动 `target/debug/project.exe`，Bevy 也会从项目资源目录加载首包资源。

### 4.2 开发期覆盖配置

当前 UI 主题和 i18n 不是纯 `AssetServer` 加载，而是通过文件系统路径读取 RON，并支持开发期热加载：

- UI 主题默认：`project/assets/ui/themes/default.ron`
- i18n 默认：`project/assets/ui/i18n/zh_cn.ron`、`project/assets/ui/i18n/en_us.ron`
- 主题覆盖环境变量：`MYBEVY_UI_THEME`
- i18n 文件覆盖环境变量：`MYBEVY_UI_I18N`
- i18n 语言覆盖环境变量：`MYBEVY_UI_LOCALE`

示例：

```powershell
$env:MYBEVY_UI_THEME="C:\tmp\my-theme.ron"
$env:MYBEVY_UI_LOCALE="en_us"
$env:MYBEVY_UI_I18N="C:\tmp\en_us.ron"
Set-Location project
cargo run
```

开发期可以直接改 RON 文件验证 UI，但正式后续下载配置仍应走内容清单、版本和哈希校验。

## 5. APK 包内资源

当前 Android 工程在 `android/app/build.gradle` 中配置：

```gradle
sourceSets {
    main {
        assets.srcDirs += files("../../project/assets")
        res.srcDirs += files("../../project/assets/android-res")
    }
}
```

含义：

- `project/assets` 会进入 APK assets，供 Bevy `AssetServer` 读取。
- `project/assets/android-res` 会进入 Android `res`，用于 launcher 图标等 Android 原生资源。
- APK assets 是只读的，运行时不能写回或替换。

Android 打包命令：

```powershell
Set-Location project
cargo ndk -t arm64-v8a -P 26 -o ..\android\app\src\main\jniLibs rustc --release --lib -- --crate-type cdylib

Set-Location ..\android
.\gradlew.bat assembleDebug
```

如果资源不应进入 APK，不要放在 `project/assets` 下面。

## 6. 后续下载资源

### 6.1 内容清单

后续下载资源必须由内容清单驱动。清单建议至少包含：

```json
{
  "version": "2026.06.09.1",
  "baseUrl": "https://cdn.example.com/mybevy/content/2026.06.09.1/",
  "assets": [
    {
      "id": "ui.theme.default",
      "kind": "ui_theme",
      "path": "ui/themes/default.ron",
      "bytes": 1941,
      "sha256": "0123456789abcdef..."
    },
    {
      "id": "props.crate",
      "kind": "gltf_scene",
      "path": "models/props/crate/crate.glb",
      "scene": 0,
      "bytes": 1048576,
      "sha256": "abcdef0123456789..."
    }
  ]
}
```

清单字段约定：

- `version`：内容版本，用于缓存隔离和回滚。
- `baseUrl`：下载根 URL。
- `id`：稳定资源 ID，业务层引用它，不直接散落路径。
- `kind`：资源类型，例如 `image`、`atlas_image`、`ui_theme`、`ui_i18n`、`font`、`audio`、`gltf_scene`、`gltf_animation`。
- `path`：版本目录下的相对路径。
- `bytes`：文件大小，用于下载后快速校验。
- `sha256`：内容哈希，用于安全和缓存一致性。

### 6.2 下载与缓存

正式流程：

1. 启动时加载首包默认资源和本地内容索引。
2. 请求远端内容清单。
3. 对比本地缓存中的版本、大小和 `sha256`。
4. 缺失或过期资源下载到临时文件。
5. 校验大小和 `sha256`。
6. 校验通过后原子移动到正式缓存目录。
7. 更新本地内容索引。
8. 再通知玩法/UI 系统加载新资源。

Android 上下载内容放应用私有 `files` 或 `cache` 目录，通常不需要外部存储权限。当前项目已有 `reqwest` 和后台 `tokio` runtime，下载、校验和写文件应在后台任务执行，不要阻塞 Bevy 主线程。

### 6.3 从缓存加载

Bevy 的默认 Android asset source 读的是 APK assets。要从下载缓存读资源，应注册单独的命名 `AssetSource`，且必须在 `DefaultPlugins` 中的 `AssetPlugin` 完成构建前注册。

示例：

```rust
#[cfg(not(target_arch = "wasm32"))]
fn register_content_cache_source(app: &mut App, cache_root: String) {
    use bevy::asset::io::{file::FileAssetReader, AssetSourceBuilder};

    app.register_asset_source(
        "content_cache",
        AssetSourceBuilder::new(move || Box::new(FileAssetReader::new(cache_root.clone()))),
    );
}
```

注册顺序：

```rust
pub fn run() {
    let mut app = App::new();

    #[cfg(not(target_arch = "wasm32"))]
    register_content_cache_source(&mut app, content_cache_root());

    app.add_plugins(DefaultPlugins.set(project_asset_plugin()))
        .add_plugins(network::NetworkPlugin)
        .add_plugins(authority::AuthorityPlugin)
        .add_plugins(myserver::MyServerPlugin)
        .add_plugins(game::GamePlugin)
        .run();
}
```

不要用 `AssetSourceBuilder::platform_default(...)` 注册 Android 下载缓存源；Android 上 platform default 会走 APK `AssetManager`，不是应用私有缓存目录。

缓存资源路径示例：

```text
content_cache://2026.06.09.1/ui/images/button_primary.png
content_cache://2026.06.09.1/models/props/crate/crate.glb
```

正式代码里不要手写版本字符串，应来自内容清单：

```rust
let asset_path = format!("content_cache://{}/{}", manifest.version, asset.path);
```

### 6.4 HTTPS 直连原型

Bevy 0.18 支持通过 `WebAssetPlugin` 直接加载 `https://...` 资源。它适合原型验证，不适合作为正式内容更新的唯一方案。

`project/Cargo.toml`：

```toml
bevy = { version = "0.18.1", features = ["https"] }
```

注册插件：

```rust
use bevy::{asset::io::web::WebAssetPlugin, prelude::*};

pub fn run() {
    App::new()
        .add_plugins(WebAssetPlugin {
            silence_startup_warning: true,
        })
        .add_plugins(DefaultPlugins.set(project_asset_plugin()))
        .run();
}
```

限制：

- URL 必须来自可信内容清单，不要加载用户输入的任意 URL。
- 直接远端加载不方便做版本回滚、断点续传、磁盘配额和哈希校验。
- `web_asset_cache` feature 只适合简单缓存，不能替代正式内容版本管理。

## 7. 资源类型使用方式

### 7.1 图片

首包图片：

```rust
fn spawn_image(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Sprite::from_image(
        asset_server.load("images/logo.png"),
    ));
}
```

UI 图片：

```rust
fn spawn_ui_image(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(ImageNode::new(
        asset_server.load("ui/images/button_primary.png"),
    ));
}
```

后续下载图片：

```rust
let image_path = format!("content_cache://{}/{}", manifest.version, "ui/images/banner.png");
let image = asset_server.load::<Image>(image_path);
```

建议：

- UI 图片优先使用 PNG。
- 大图和活动图应走后续下载。
- 移动端控制尺寸，避免 4096 以上纹理无必要进入首包。

### 7.2 图集

规则网格图集：

```rust
fn setup_atlas(
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = asset_server.load("ui/atlas/icons.png");
    let layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(32, 32),
        8,
        8,
        None,
        None,
    ));

    let atlas = TextureAtlas {
        layout,
        index: 0,
    };

    // `image` 和 `atlas` 可存入资源，供 UI 或 Sprite 使用。
    let _ = (image, atlas);
}
```

UI 图集：

```rust
fn spawn_atlas_icon(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = asset_server.load("ui/atlas/icons.png");
    let layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(32, 32),
        8,
        8,
        None,
        None,
    ));

    commands.spawn(ImageNode::from_atlas_image(
        image,
        TextureAtlas { layout, index: 3 },
    ));
}
```

图集建议：

- 图集图片和图集元数据必须同源。首包图集都在 `project/assets`；后续下载图集都在同一个 `content_cache://<version>/...` 下。
- 如果不是规则网格，建议新增 RON/JSON 图集描述文件，并纳入同一内容清单。
- 图集描述更新时，图集图片和描述文件版本必须一起更新。

### 7.3 字体

当前 UI 字体首包路径：

```text
project/assets/ui/fonts/MyBevyUiCjk-Regular.otf
```

当前代码加载方式：

```rust
let font = asset_server.load("ui/fonts/MyBevyUiCjk-Regular.otf");
```

字体建议：

- 基础 UI 字体进首包，保证离线和首屏可用。
- 运营活动字体或皮肤字体可走后续下载，但必须有首包 fallback 字体。
- 字体授权文件要随资源保留，例如当前 `NotoSansCJKsc-LICENSE.txt`。

### 7.4 UI 主题与 i18n

当前主题配置：

```text
project/assets/ui/themes/default.ron
```

当前 i18n 配置：

```text
project/assets/ui/i18n/zh_cn.ron
project/assets/ui/i18n/en_us.ron
```

开发期支持环境变量覆盖和热加载，见第 4.2 节。

后续下载 UI 配置建议：

- 把主题和 i18n RON 文件纳入内容清单。
- 下载并校验后，从缓存路径读取。
- 每个配置文件保留 `version` 字段，解析时校验兼容性。
- 配置加载失败时回退到首包默认主题和默认中文文案。

注意：当前实现使用文件系统路径读取 UI 主题和 i18n。如果要让它们正式支持 `content_cache://...`，需要在 UI 配置加载模块中增加“内容缓存文件路径”或“AssetServer 文本资源加载”支持。

### 7.5 音频

当前音频框架位于 `project/src/framework/audio/`，首包音频放在 `project/assets/audio/`。当前样例资源主要是 `.wav`，`project/Cargo.toml` 已显式开启 Bevy `wav` feature：

```toml
bevy = { version = "0.18.1", features = ["wav"] }
```

首包音频路径从 `project/assets/` 下一级开始写：

```rust
fn play_sound(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(AudioPlayer::new(
        asset_server.load("audio/ui/click_wood_01.wav"),
    ));
}
```

后续下载音频：

```rust
let path = format!("content_cache://{}/{}", manifest.version, "audio/music/event_theme.wav");
let audio = asset_server.load::<AudioSource>(path);
```

建议：

- 短 UI 音效、关键反馈音和小体积默认 ambience 可进首包。
- 背景音乐、活动语音、大体积音频优先走后续下载。
- 当前首包音频目录包含 `ui/`、`common/`、`ambience/`、`music/`、`battle/`、`voice/`、`spatial/`，新增路径必须保持小写和 Android 大小写一致。
- 当前已验证和显式启用的是 `.wav`；新增 OGG/Vorbis、MP3、FLAC、AAC 等格式前，先确认 Bevy feature 和目标平台支持。
- `project/assets/audio/readme.md` 中的现有文件是开发期占位资源；公开发布前应替换为自有、委托制作或明确可再分发的音频，并记录授权。
- `AudioCatalogConfig` 已允许 `content_cache://...` 路径，但真实下载、缓存源注册和版本哈希校验仍是后续目标；不要把后续下载音频直接放进 `project/assets/`。

### 7.6 3D 模型和场景

推荐格式：

- 推荐：`.glb`
- 可用：`.gltf` + `.bin` + 外部贴图
- 不推荐直接运行时加载：`.fbx`、`.blend`、`.obj`

首包 3D 场景：

```rust
fn spawn_model(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        SceneRoot(
            asset_server.load(
                GltfAssetLabel::Scene(0).from_asset("models/props/crate/crate.glb"),
            ),
        ),
        Transform::default(),
        Name::new("CrateModel"),
    ));
}
```

后续下载 3D 场景：

```rust
let path = format!(
    "content_cache://{}/{}",
    manifest.version,
    "models/props/crate/crate.glb"
);
let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
```

单个 mesh primitive：

```rust
let mesh = asset_server.load(
    GltfAssetLabel::Primitive {
        mesh: 0,
        primitive: 0,
    }
    .from_asset("models/props/crate/crate.glb"),
);
```

动画片段：

```rust
let animation = asset_server.load(
    GltfAssetLabel::Animation(0).from_asset("models/characters/hero/hero.glb"),
);
```

3D 美术检查：

- Bevy 和 glTF 都使用 Y 轴向上的 3D 坐标习惯。
- 按米制作，避免导入后极端缩放。
- 导出前应用缩放和旋转。
- 贴图尺寸按移动端预算控制。
- `.gltf` 的所有依赖文件必须与主文件保持相对路径一致，并进入同一清单。

### 7.7 配置和数据文件

RON/JSON 等配置文件分两类：

- 启动必需配置：进首包，并提供内置 fallback。
- 运营和玩法数据：走内容清单和缓存。

建议：

- 每个配置文件有 `version` 字段。
- 解析失败不能让应用崩溃，应记录错误并使用上一版缓存或首包默认值。
- 不要把不可信远端配置直接反序列化后驱动危险操作；先做字段校验和范围限制。

### 7.8 场景 CSV、Manifest 和 Layout

当前样板场景使用三类首包文本数据：

- `project/assets/game/scenes.csv`：游戏层场景目录表，当前包含 `sample.dungeon_room`。它只描述场景是否启用、展示排序、标题/描述 key、`kind`、manifest path、layout path、默认 spawn 和 UI 模式，不写 prefab 坐标。
- `project/assets/scenes/sample_dungeon_room/scene.ron`：framework manifest，声明场景 ID、相机、Loading 策略、资源 layer、spawn point 和 anchors。manifest 中的 glTF 只会被框架加载跟踪，不会自动摆放到世界。
- `project/assets/scenes/sample_dungeon_room/layout.ron`：game layer layout，声明样板房间的 prefab 和 light 实例。当前由 `project/src/game/scenes/sample_dungeon_room.rs` 在 `SceneEvent::Entered` 后读取并实例化。

这三类路径都按首包资源规则书写，也就是相对 `project/assets/`。后续下载版本的 CSV、manifest 或 layout 仍未接入 `content_cache://...`。

场景 manifest 当前可把 `audio` / `sound` 作为 asset kind 进行加载跟踪，但不会自动注册 audio catalog 或播放音频。具体场景音频仍由 `project/src/framework/audio/scene.rs` 的 `SceneAudioAdapterConfig` 和 game layer 注册决定。

## 8. 加载状态与页面时机

Bevy 的资产加载是异步的。`asset_server.load(...)` 返回 handle 时，资源通常还没完全可用。

常见策略：

- 简单图片、字体、音频：保存 handle，Bevy 就绪后自动显示或播放。
- 3D `SceneRoot`：直接生成根实体，Bevy 加载完成后自动实例化场景层级。
- 需要读 glTF 内部信息：先加载 `Handle<Gltf>`，再查询 `Assets<Gltf>`。
- 需要等场景实例化后改材质、挂组件或启动动画：使用 `SceneInstanceReady`。
- 后续下载资源：下载和校验完成后再调用 `asset_server.load(...)`。

页面状态建议：

1. 显示首包占位资源或 Loading UI。
2. 请求内容清单。
3. 下载并校验缺失资源。
4. 加载缓存资源。
5. 加载成功后切换正式内容。
6. 失败时使用旧缓存或首包 fallback。

### 8.1 方圆 RON 与 Bake Artifact 加载诊断

方圆开发源仍是 RON；当前 bake artifact 使用 `.fyb` 扩展名，格式是 `FYBAKE` 自定义 header + typed payload。运行时 loader 优先尝试 bin，debug 配置允许 RON fallback，release 配置不允许 fallback。

一键 dry-run：

```powershell
.\scripts\run-fangyuan-bake-dry-run.ps1
```

报告默认写入：

```text
artifacts/fangyuan-bake/dry-run/report.txt
```

报告字段含义：

- `source_bytes`：源 RON 文件字节数。
- `artifact_size`：当前 `.fyb` artifact 估算字节数。
- `peak_resource_count`：本条 artifact 统计到的 primitive / prefab / chunk / profile 峰值数量。
- `ron_load_us`：开发机上 RON parse、版本升级、校验和 payload 编译耗时。
- `bin_load_us`：开发机内存中 `.fyb` header、schema、kind、hash 和 typed payload decode 耗时。

这些字段用于比较两条加载路径的错误处理和数据规模差异，不是手机真机性能数据。当前 RON 路径主要暴露 parse、upgrade、validator 和依赖发现错误；bin 路径额外暴露 magic、schema version、artifact kind、content/source hash、payload version 和依赖缺失错误。

当前仓库没有 `.github` CI workflow，也没有通用本地检查脚本。后续接入 CI 或提交前检查时，应追加 `.\scripts\run-fangyuan-bake-dry-run.ps1`，并把 report 作为构建日志/产物保存。

## 9. 常见问题

首包资源找不到：

- 代码路径是否从 `project/assets/` 下一级开始写。
- 文件是否真的在 `project/assets` 下。
- 大小写是否完全一致。
- Android 上是否误放到了 `project/assets/android-res` 而不是 `project/assets`。

后续下载资源找不到：

- 资源是否已下载完成并通过大小和 `sha256` 校验。
- 是否在 `AssetPlugin` 前注册了 `content_cache`。
- 路径是否使用 `content_cache://<version>/...`。
- 缓存根目录是否稳定指向 `<app_private_cache>/mybevy-content`。
- Android 上是否误用了 APK asset source 读取应用缓存目录。

UI 配置没有更新：

- 是否设置了正确的 `MYBEVY_UI_THEME` 或 `MYBEVY_UI_I18N`。
- RON 文件 `version` 是否匹配当前代码支持的版本。
- 热加载轮询是否读到的是你正在编辑的文件。
- 后续下载配置是否已经写入 UI 配置模块实际读取的文件路径。

贴图或 glTF 依赖丢失：

- 优先导出 `.glb`。
- 如果使用 `.gltf`，确认 `.bin` 和贴图也在同一资源源下。
- 后续下载时，依赖文件都要进入内容清单。

## 10. 提交前检查

修改 Rust 资源加载代码后：

```powershell
Set-Location project
cargo fmt
cargo check
```

只新增资源或文档时，至少确认：

- 首包资源确实应该进入 `project/assets`。
- 二进制首包资源已命中 Git LFS 规则，可用 `git check-attr filter -- <path>` 检查。
- 后续下载资源没有误放进 `project/assets`。
- 方圆 `.fyb` 只有在明确作为首包发布资源时才放入 `project/assets`，否则留在 `artifacts/`、`target/` 或构建系统产物目录。
- Android 首包资源路径能被 APK assets 读取。
- 后续资源清单里的路径、大小、哈希和实际文件一致。
- UI 配置 RON 能解析，版本字段正确。
- 字体、图片、音频、模型授权允许随项目发布或下载分发。
- 没有提交 `target/`、临时导出目录或过大的源工程文件。

## 11. 落地顺序

接入首包资源：

1. 确认资源是启动或基础体验必需。
2. 放入 `project/assets/<type>/...`。
3. 用相对资源路径加载，例如 `ui/fonts/...`。
4. 在桌面端运行验证。
5. 构建 APK 验证 Android 读取。

接入后续下载资源：

1. 放入 `content_dist/<version>/<type>/...`，不要放入 `project/assets`。
2. 生成内容清单，包含 `id`、`kind`、`path`、`bytes`、`sha256`。
3. 客户端下载到临时文件并校验。
4. 原子移动到 `<app_private_cache>/mybevy-content/<version>/...`。
5. 用 `content_cache://<version>/...` 加载。
6. 为失败情况准备旧缓存或首包 fallback。
