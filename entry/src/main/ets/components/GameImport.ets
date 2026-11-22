// One-time import of the player's Yuri's Revenge installation.
//
// The document picker grants access through folder URIs. Enumeration has to
// use the URI's path portion (listFileSync rejects the "file://docs/..." form),
// while reads use the granted child URIs; the files are copied once into the
// app's private storage and config.toml points the engine at that folder.

import { fileIo, fileUri, picker, ListFileOptions, WriteOptions } from '@kit.CoreFileKit';
import { BusinessError } from '@kit.BasicServicesKit';

const ASSET_DIR_NAME: string = 'Game';
const REQUIRED_ARCHIVES: string[] = ['ra2md.mix', 'ra2.mix'];
// Startup treats movie archives as optional media; they are the bulk of an
// installation and would stretch the one-time copy by gigabytes.
const SKIPPED_MIXES: string[] = ['movies01.mix', 'movies02.mix', 'movmd03.mix'];
const SKIPPED_SUFFIXES: string[] = ['.exe', '.dll', '.lnk', '.url', '.log', '.txt', '.dat'];
const COPY_CHUNK_BYTES: number = 1024 * 1024;

export function assetsDir(filesDir: string): string {
  return filesDir + '/' + ASSET_DIR_NAME;
}

function baseName(name: string): string {
  const slash = Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\'));
  return slash >= 0 ? name.substring(slash + 1) : name;
}

function joinPath(folder: string, name: string): string {
  return folder.endsWith('/') ? folder + name : folder + '/' + name;
}

function isSkipped(name: string): boolean {
  const lower = baseName(name).toLowerCase();
  if (lower.length === 0 || lower.startsWith('.')) {
    return true;
  }
  for (let i = 0; i < SKIPPED_MIXES.length; i++) {
    if (lower === SKIPPED_MIXES[i]) {
      return true;
    }
  }
  for (let i = 0; i < SKIPPED_SUFFIXES.length; i++) {
    if (lower.endsWith(SKIPPED_SUFFIXES[i])) {
      return true;
    }
  }
  return false;
}

function listNames(folder: string): string[] {
  const options: ListFileOptions = { recursion: false };
  return fileIo.listFileSync(folder, options);
}

function describeError(error: BusinessError): string {
  if (Number.isInteger(error.code) && error.code !== 0) {
    return error.message + '（错误码 ' + error.code + '）';
  }
  return error.message;
}

function hasRequiredArchives(names: string[]): boolean {
  let seen: number = 0;
  for (let i = 0; i < names.length; i++) {
    const lower = baseName(names[i]).toLowerCase();
    for (let r = 0; r < REQUIRED_ARCHIVES.length; r++) {
      if (lower === REQUIRED_ARCHIVES[r]) {
        seen |= 1 << r;
      }
    }
  }
  return seen === (1 << REQUIRED_ARCHIVES.length) - 1;
}

function writeGameConfig(configDir: string, ra2Dir: string): void {
  const path = configDir + '/config.toml';
  const content = '[paths]\nra2_dir = ' + JSON.stringify(ra2Dir) + '\n';
  const file = fileIo.openSync(
    path,
    fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC
  );
  try {
    fileIo.writeSync(file.fd, content);
  } finally {
    fileIo.closeSync(file);
  }
}

/// Whether the engine already has its archives; writes config.toml when they
/// are there but the configuration is not (an earlier install, or a folder
/// filled by hand). `configDir` is the application-level files directory the
/// native host runs in; `legacyDir` is the module directory older imports
/// used, whose archives stay usable in place.
export function prepareGameFiles(configDir: string, legacyDir: string): boolean {
  for (let i = 0; i < 2; i++) {
    const dir = assetsDir(i === 0 ? configDir : legacyDir);
    try {
      if (!fileIo.accessSync(dir)) {
        continue;
      }
      if (!hasRequiredArchives(listNames(dir))) {
        continue;
      }
      writeGameConfig(configDir, dir);
      return true;
    } catch {
    }
  }
  return false;
}

@Component
export struct GameImport {
  onReady: () => void = () => {};
  @State phase: string = 'idle';
  @State status: string = '尚未选择文件夹';
  @State detail: string = '';
  @State progress: number = 0;
  @State copiedMb: number = 0;

  private fail(text: string, detail: string): void {
    this.phase = 'error';
    this.status = text;
    this.detail = detail;
  }

  private async chooseFolder(): Promise<void> {
    if (this.phase === 'copying') {
      return;
    }
    const context = this.getUIContext().getHostContext();
    if (!context) {
      this.fail('无法获取应用上下文', '');
      return;
    }
    let uris: string[] = [];
    try {
      const documentPicker = new picker.DocumentViewPicker(context);
      const options = new picker.DocumentSelectOptions();
      options.maxSelectNumber = 1;
      options.selectMode = picker.DocumentSelectMode.FOLDER;
      // Open in Download, the usual home of the game folder.
      options.defaultFilePathUri = 'file://docs/storage/Users/currentUser/Download';
      uris = await documentPicker.select(options);
    } catch (error) {
      const e = error as BusinessError;
      this.fail('打开文件夹选择器失败', describeError(e));
      return;
    }
    if (uris.length === 0) {
      return;
    }
    // The native host runs in the application-level files directory, which is
    // not the module directory this UIAbility context points at.
    await this.importFolder(context.getApplicationContext().filesDir, uris[0]);
  }

  /// The picker's path is what listFileSync understands; the URI itself is
  /// only valid for opening the folder's children.
  private folderPathOf(folderUri: string): string {
    try {
      return new fileUri.FileUri(folderUri).path;
    } catch {
      return '';
    }
  }

  private async importFolder(filesDir: string, folderUri: string): Promise<void> {
    this.phase = 'copying';
    this.progress = 0;
    this.copiedMb = 0;
    this.detail = '';
    this.status = '正在读取游戏文件夹…';

    const folderPath = this.folderPathOf(folderUri);
    const candidates: string[] = [];
    if (folderPath.length > 0) {
      candidates.push(folderPath);
    }
    candidates.push(folderUri);

    let names: string[] = [];
    let errorText = '';
    for (let i = 0; i < candidates.length; i++) {
      try {
        names = listNames(candidates[i]);
        errorText = '';
        break;
      } catch (error) {
        const e = error as BusinessError;
        errorText += (errorText.length > 0 ? '；' : '') + describeError(e);
      }
    }
    if (errorText.length > 0) {
      this.fail('无法读取所选文件夹', '路径 ' + candidates.join('／') + '：' + errorText);
      return;
    }
    if (!hasRequiredArchives(names)) {
      this.fail(
        '所选文件夹不是《尤里的复仇》安装目录：未找到 ra2md.mix 和 ra2.mix',
        '扫描到 ' + names.length + ' 个文件，其中没有所需的存档包'
      );
      return;
    }

    const selected: string[] = [];
    for (let i = 0; i < names.length; i++) {
      if (!isSkipped(names[i])) {
        selected.push(names[i]);
      }
    }
    if (selected.length === 0) {
      this.fail('所选文件夹没有可导入的文件', '');
      return;
    }

    const dest = assetsDir(filesDir);
    try {
      if (!fileIo.accessSync(dest)) {
        fileIo.mkdirSync(dest, true);
      }
    } catch (error) {
      const e = error as BusinessError;
      this.fail('无法创建导入目录', describeError(e));
      return;
    }

    const buffer = new ArrayBuffer(COPY_CHUNK_BYTES);
    let copiedBytes: number = 0;
    let failedFile: string = '';
    for (let i = 0; i < selected.length; i++) {
      const name = selected[i];
      const label = baseName(name);
      this.status = '正在导入 ' + (i + 1) + '/' + selected.length + '：' + label;
      try {
        copiedBytes += await this.copyEntry(
          folderUri,
          folderPath,
          label,
          dest + '/' + label,
          buffer
        );
        this.copiedMb = Math.floor(copiedBytes / (1024 * 1024));
      } catch (error) {
        const e = error as BusinessError;
        // Directories and unavailable entries fail to open; only a required
        // archive failing matters.
        if (REQUIRED_ARCHIVES.indexOf(label.toLowerCase()) >= 0) {
          failedFile = label + '：' + describeError(e);
          break;
        }
      }
      this.progress = Math.floor((i + 1) * 100 / selected.length);
    }

    if (failedFile.length > 0) {
      this.fail('导入必需的存档包失败', failedFile);
      return;
    }
    try {
      if (!hasRequiredArchives(listNames(dest))) {
        this.fail('导入后缺少 ra2md.mix 或 ra2.mix，请重试', '');
        return;
      }
      writeGameConfig(filesDir, assetsDir(filesDir));
    } catch (error) {
      const e = error as BusinessError;
      this.fail('写入 config.toml 失败', describeError(e));
      return;
    }

    this.phase = 'done';
    this.progress = 100;
    this.status = '导入完成（' + this.copiedMb + ' MB），正在启动 VERA20K…';
    setTimeout(() => {
      this.onReady();
    }, 600);
  }

  /// Reads through the granted URI first; the plain path is the fallback for
  /// folders reached without a document grant.
  private async copyEntry(
    folderUri: string,
    folderPath: string,
    label: string,
    target: string,
    buffer: ArrayBuffer
  ): Promise<number> {
    let firstError: Error = new Error('没有可读取的来源：' + label);
    if (folderUri.startsWith('file://')) {
      try {
        return await this.copyFile(joinPath(folderUri, label), target, buffer);
      } catch (error) {
        firstError = error as Error;
      }
    }
    if (folderPath.length > 0) {
      try {
        return await this.copyFile(joinPath(folderPath, label), target, buffer);
      } catch {
        // Keep the URI attempt's error: it carries the granted-access reason.
      }
    }
    throw firstError;
  }

  private async copyFile(source: string, target: string, buffer: ArrayBuffer): Promise<number> {
    const src = fileIo.openSync(source, fileIo.OpenMode.READ_ONLY);
    let dst: fileIo.File | undefined = undefined;
    try {
      dst = fileIo.openSync(
        target,
        fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC
      );
      let total: number = 0;
      while (true) {
        const read = await fileIo.read(src.fd, buffer);
        if (read <= 0) {
          break;
        }
        const options: WriteOptions = { length: read };
        await fileIo.write(dst.fd, buffer, options);
        total += read;
      }
      return total;
    } finally {
      fileIo.closeSync(src);
      if (dst !== undefined) {
        fileIo.closeSync(dst);
      }
    }
  }

  build() {
    Column({ space: 18 }) {
      Text('VERA20K')
        .fontSize(40)
        .fontWeight(FontWeight.Bold)
        .fontColor('#FFD700')
      Text('需要《红色警戒 2：尤里的复仇》游戏文件')
        .fontSize(22)
        .fontColor('#E8E8E8')
      Text('选择一个包含 ra2md.mix 与 ra2.mix 的安装文件夹（游戏文件夹通常在“下载”里）。文件会被复制到应用私有存储，原文件夹不会被修改；电影包体积过大，不参与导入。')
        .fontSize(14)
        .fontColor('#B9B9B9')
        .width(620)
        .textAlign(TextAlign.Center)

      Button(this.phase === 'copying' ? '正在导入…' : '选择游戏文件夹…')
        .width(280)
        .height(44)
        .fontSize(16)
        .backgroundColor('#7A1F1F')
        .fontColor('#FFD700')
        .enabled(this.phase !== 'copying' && this.phase !== 'done')
        .onClick(() => {
          this.chooseFolder();
        })

      if (this.phase === 'copying' || this.phase === 'done') {
        Progress({ value: this.progress, total: 100, type: ProgressType.Linear })
          .width(560)
          .color('#FFD700')
          .backgroundColor('#333333')
        Text(this.status)
          .fontSize(14)
          .fontColor('#D9D9D9')
          .width(620)
          .maxLines(3)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .textAlign(TextAlign.Center)
      } else {
        Text(this.status)
          .fontSize(14)
          .fontColor(this.phase === 'error' ? '#FF7070' : '#999999')
          .width(620)
          .textAlign(TextAlign.Center)
        if (this.detail.length > 0) {
          Text(this.detail)
            .fontSize(12)
            .fontColor('#9A9A9A')
            .width(620)
            .maxLines(4)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .textAlign(TextAlign.Center)
        }
      }
    }
    .width('100%')
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .backgroundColor('#101010')
  }
}