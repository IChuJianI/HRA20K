# 第三方软件声明

HRA20K 以 GNU GPL v3 发布，全文见 [LICENSE](LICENSE)。应用内启动画面与「许可」页显示版权、分享许可、无担保声明以及上游与本移植版的源码地址；本文件补充第三方组件、版权与许可信息。

## VERA20K 引擎

- 来源：https://github.com/YuriPlanet/vera20k
- 许可证：GNU GPL v3（[vera20k-main/LICENSE-GPL](vera20k-main/LICENSE-GPL)）及附加条款（[vera20k-main/LEGAL.md](vera20k-main/LEGAL.md)）
- 版权：Copyright © 2026 Halvor Nikolai Rusten and VERA20K contributors
- 说明：HRA20K 是 VERA20K 的 HarmonyOS 2in1 修改版，非官方版本。上游附加条款中的名称使用限制与非商业请求随源代码原样保留。

## Ares

- 来源：https://github.com/Ares-Developers/Ares
- 许可证：Ares Contributors 许可（BSD 风格，含致谢条款），全文见 [vera20k-main/LICENSES/Ares.txt](vera20k-main/LICENSES/Ares.txt)
- 版权：Copyright (c) 2008+, All Ares Contributors
- 致谢：This product includes software developed by the Ares Contributors.
- 说明：可选 Ares 支持的部分代码译自 Ares；在 VERA20K 中按 GPLv3 分发，同时保留 Ares 的版权与许可声明。

## Phobos

- 来源：https://github.com/Phobos-developers/Phobos
- 许可证：GPLv3
- 版权：Phobos Developers
- 致谢：This product includes software developed by the Phobos Developers.

## FFmpeg（Bink 视频解码器、xBR 缩放器）

- 来源：https://ffmpeg.org
- 许可证：GPLv3（上游对应部分为 LGPL 2.1，按其第 3 节转为 GPLv3 使用）
- 版权：FFmpeg developers
- 说明：解码器与缩放器为移植到 Rust 的实现；GPLv3 全文由本仓库的 [LICENSE](LICENSE) 覆盖。

## 界面字体

- Noto Sans 与 Noto Sans CJK：来源 https://github.com/notofonts ，以 SIL Open Font License 1.1 未修改分发
- GNU Unifont（`Unifont-CJK-15.bin`）：来源 https://unifoundry.com/unifont/ ，以 SIL OFL 1.1 分发
- 许可见 `vera20k-main/src/render/fonts/OFL.txt`、`OFL-CJK.txt`、`OFL-Unifont.txt`，并随应用打包于 `entry/src/main/resources/rawfile/licenses/`

## Rust 引擎运行时

- 说明：引擎以 Rust 编写，栈展开等运行时以 LLVM libunwind 实现并静态链入 `libvera20k_ohos.so`；签名 HAP 只包含这一个引擎库，不含独立的 `libc++_shared.so`，本仓库也不包含 Rust 或 LLVM 的二进制文件。
- 许可证：Rust 采用 MIT / Apache-2.0；LLVM libunwind 采用 Apache-2.0 WITH LLVM-exception（该例外允许嵌入对象形式不附带许可证副本）。
- 版权：The Rust Project Developers；LLVM Project contributors
- 许可见 https://www.rust-lang.org/policies/licenses 与 https://llvm.org/LICENSE.txt 。

## 游戏文件

VERA20K 与 HRA20K 均不包含《红色警戒2：尤里的复仇》的任何游戏文件。游戏文件版权归 Electronic Arts 所有，本项目的许可不覆盖它们；请自备正版游戏文件。

程序图标（《红色警戒2》的红星）与宣传截图展示的是 Electronic Arts 所有的游戏美术，本项目的许可同样不覆盖它们。HRA20K 与 Electronic Arts 无隶属关系，也未获其认可或授权。

## 完整文本位置

- GNU GPL v3：`vera20k-main/LICENSE-GPL`（应用内 `rawfile/licenses/LICENSE-GPL`）
- VERA20K 附加条款与三方声明：`vera20k-main/LEGAL.md`（应用内 `rawfile/licenses/LEGAL.md`）
- Ares 许可：`vera20k-main/LICENSES/Ares.txt`（应用内 `rawfile/licenses/Ares.txt`）
- 字体 OFL：见上
