import { bundleManager, common } from '@kit.AbilityKit';
import { fileIo } from '@kit.CoreFileKit';
import { pasteboard } from '@kit.BasicServicesKit';
import { i18n } from '@kit.LocalizationKit';
import { window } from '@kit.ArkUI';
import { GameImport, prepareGameFiles } from '../components/GameImport';

@Entry
@Component
struct Index {
  @State engineReady: boolean = false;
  // The random-map dialog's Paste row, in vp; the system PasteButton covers
  // it while the engine publishes it (the row rect comes in surface px).
  @State pasteVisible: boolean = false;
  @State pasteX: number = 0;
  @State pasteY: number = 0;
  @State pasteW: number = 0;
  @State pasteH: number = 0;
  private mainWindow: window.Window | undefined = undefined;
  private watchTimer: number = -1;
  private windowActive: boolean = true;
  private blurRetry: number = -1;
  private blurRetryB: number = -1;
  private focusRetryA: number = -1;
  private focusRetryB: number = -1;
  private exiting: boolean = false;
  private filesDir: string = '';

  aboutToAppear(): void {
    const context = this.getUIContext().getHostContext();
    // The native host's working directory is the application-level files
    // directory; the module files directory only carries the log mirror.
    if (context &&
      prepareGameFiles(context.getApplicationContext().filesDir, context.filesDir)) {
      this.engineReady = true;
    }
    if (context) {
      this.filesDir = context.getApplicationContext().filesDir;
      this.publishSystemLocale(this.filesDir);
      this.clearFile(this.filesDir + '/vera20k.pastezone');
      this.clearFile(this.filesDir + '/vera20k.pastecode');
      this.clearFile(this.filesDir + '/vera20k.pastecode.tmp');
      this.publishWindowId(this.filesDir);
      this.watchSignals(this.filesDir);
    }
  }

  aboutToDisappear(): void {
    if (this.watchTimer >= 0) {
      clearInterval(this.watchTimer);
      this.watchTimer = -1;
    }
    this.clearFocusTimers();
  }

  // The native engine has no LANG in this sandbox, so the page publishes the
  // system's preferred languages; the game follows them until a language is
  // chosen under Options.
  private publishSystemLocale(filesDir: string): void {
    let tags: string[] = [];
    try {
      tags = i18n.System.getPreferredLanguageList();
    } catch {
    }
    if (tags.length === 0) {
      try {
        tags = [i18n.System.getSystemLanguage()];
      } catch {
      }
    }
    try {
      const file = fileIo.openSync(filesDir + '/vera20k.locale',
        fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
      fileIo.writeSync(file.fd, tags.join('\n'));
      fileIo.closeSync(file);
    } catch {
    }
  }

  private clearFile(path: string): void {
    try {
      if (fileIo.accessSync(path)) {
        fileIo.unlinkSync(path);
      }
    } catch {
    }
  }

  // The game's cursor APIs need the main window id, which only ArkTS can
  // read. Drop last run's file first so the native side never reads a stale
  // id during this run's start-up. The same window keeps the game full
  // screen: it has no windowed mode.
  private publishWindowId(filesDir: string): void {
    const path = filesDir + '/vera20k.windowid';
    this.clearFile(path);
    const context = this.getUIContext().getHostContext();
    if (!context) {
      return;
    }
    window.getLastWindow(context).then((mainWindow: window.Window) => {
      this.mainWindow = mainWindow;
      try {
        const id = mainWindow.getWindowProperties().id;
        const file = fileIo.openSync(path,
          fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
        fileIo.writeSync(file.fd, id.toString());
        fileIo.closeSync(file);
      } catch {
      }
      // Esc is the system's own "leave immersive" shortcut, so the window
      // manager must have no windowed mode to fall back to: the ability
      // supports full screen only (module.json5 says the same), and the
      // floating state below is only a last-resort repair.
      this.lockFullScreenOnly(mainWindow);
      try {
        mainWindow.on('windowEvent', (event: window.WindowEventType) => {
          if (event === window.WindowEventType.WINDOW_ACTIVE) {
            this.windowActive = true;
            this.restoreSurfaceFocus();
          } else if (event === window.WindowEventType.WINDOW_INACTIVE) {
            this.windowActive = false;
          }
        });
      } catch {
      }
      try {
        mainWindow.on('windowStatusChange', (status: window.WindowStatusType) => {
          if (this.needsFullScreen(status)) {
            this.enterFullScreen(mainWindow);
          }
        });
      } catch {
      }
      this.enterFullScreen(mainWindow);
    }).catch(() => {
    });
  }

  // The manifest declares the same; doing it at runtime as well keeps the
  // window manager from ever offering a floating state for this ability.
  private lockFullScreenOnly(mainWindow: window.Window): void {
    try {
      mainWindow.setSupportedWindowModes([bundleManager.SupportWindowMode.FULL_SCREEN]).catch(() => {
      });
    } catch {
    }
  }

  private needsFullScreen(status: window.WindowStatusType): boolean {
    return status === window.WindowStatusType.FLOATING;
  }

  // The title bar and the dock never surface over the game: reaching the top
  // or bottom hot zone while dragging the map must not pop them in, so the
  // immersive presentation disables both hovers.
  private enterFullScreen(mainWindow: window.Window): void {
    if (this.exiting) {
      return;
    }
    try {
      mainWindow.maximize(
        window.MaximizePresentation.ENTER_IMMERSIVE_DISABLE_TITLE_AND_DOCK_HOVER)
        .then(() => {
          mainWindow.setTitleAndDockHoverShown(false, false).catch(() => {
          });
          this.focusSurfaceNow();
        })
        .catch(() => {
        });
    } catch {
    }
  }

  // A screenshot or recorder overlay blurs the surface, and the window can
  // regain activation without ArkUI restoring component focus. The engine
  // drops every input while it believes the window is inactive (the match
  // itself keeps running), so put the focus back explicitly.
  private restoreSurfaceFocus(): void {
    this.focusSurfaceNow();
    if (this.focusRetryA >= 0) {
      clearTimeout(this.focusRetryA);
    }
    if (this.focusRetryB >= 0) {
      clearTimeout(this.focusRetryB);
    }
    this.focusRetryA = setTimeout(() => {
      this.focusRetryA = -1;
      this.focusSurfaceNow();
    }, 150);
    this.focusRetryB = setTimeout(() => {
      this.focusRetryB = -1;
      this.focusSurfaceNow();
    }, 500);
  }

  // A transient system window can blur the surface while this window stays
  // focused; nothing focused it back once the overlay closed.
  private recoverSurfaceFocus(): void {
    if (this.exiting || this.pasteVisible) {
      return;
    }
    if (this.blurRetry >= 0) {
      clearTimeout(this.blurRetry);
    }
    if (this.blurRetryB >= 0) {
      clearTimeout(this.blurRetryB);
    }
    this.blurRetry = setTimeout(() => {
      this.blurRetry = -1;
      this.focusSurfaceIfActive();
    }, 300);
    this.blurRetryB = setTimeout(() => {
      this.blurRetryB = -1;
      this.focusSurfaceIfActive();
    }, 1000);
  }

  private focusSurfaceIfActive(): void {
    if (this.exiting || this.pasteVisible) {
      return;
    }
    const mainWindow = this.mainWindow;
    let active = this.windowActive;
    if (mainWindow) {
      try {
        active = mainWindow.isFocused();
      } catch {
      }
    }
    if (active) {
      this.focusSurfaceNow();
    }
  }

  private focusSurfaceNow(): void {
    try {
      focusControl.requestFocus('vera20k_surface');
    } catch {
    }
  }

  private clearFocusTimers(): void {
    if (this.blurRetry >= 0) {
      clearTimeout(this.blurRetry);
      this.blurRetry = -1;
    }
    if (this.blurRetryB >= 0) {
      clearTimeout(this.blurRetryB);
      this.blurRetryB = -1;
    }
    if (this.focusRetryA >= 0) {
      clearTimeout(this.focusRetryA);
      this.focusRetryA = -1;
    }
    if (this.focusRetryB >= 0) {
      clearTimeout(this.focusRetryB);
      this.focusRetryB = -1;
    }
  }

  private watchSignals(filesDir: string): void {
    const exitPath = filesDir + '/vera20k.exit';
    try {
      const file = fileIo.openSync(exitPath,
        fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
      fileIo.closeSync(file);
    } catch {
    }
    this.watchTimer = setInterval(() => {
      let exit = '';
      try {
        exit = fileIo.readTextSync(exitPath).trim();
      } catch {
      }
      if (exit === '1') {
        this.quitToDesktop();
        return;
      }
      this.pollPasteZone();
      // The status listener is the fast path; this catches a status the
      // platform changed without notifying.
      const mainWindow = this.mainWindow;
      if (mainWindow) {
        try {
          this.windowActive = mainWindow.isFocused();
          if (this.needsFullScreen(mainWindow.getWindowStatus())) {
            this.enterFullScreen(mainWindow);
          }
        } catch {
        }
      }
    }, 250);
  }

  // The engine publishes the random-map dialog's Paste row rect while the
  // Extras page shows; laying the system PasteButton over that row gives the
  // clipboard read the player's tap as authorization, so no restricted
  // permission is needed.
  private pollPasteZone(): void {
    const path = this.filesDir + '/vera20k.pastezone';
    let text = '';
    try {
      text = fileIo.readTextSync(path).trim();
    } catch {
      this.pasteVisible = false;
      return;
    }
    const parts = text.split(' ');
    if (parts.length !== 4) {
      this.pasteVisible = false;
      return;
    }
    const ui = this.getUIContext();
    const values: number[] = [];
    for (const part of parts) {
      const value = Number(part);
      if (!Number.isFinite(value)) {
        this.pasteVisible = false;
        return;
      }
      values.push(ui.px2vp(value));
    }
    if (values[2] <= 0 || values[3] <= 0) {
      this.pasteVisible = false;
      return;
    }
    this.pasteX = values[0];
    this.pasteY = values[1];
    this.pasteW = values[2];
    this.pasteH = values[3];
    this.pasteVisible = true;
  }

  // The native watcher replays the press on the Paste row once this file
  // lands; an empty text still reaches it so the dialog reports the miss.
  private handlePasteClick(result: PasteButtonOnClickResult): void {
    let text = '';
    if (result === PasteButtonOnClickResult.SUCCESS) {
      try {
        text = pasteboard.getSystemPasteboard().getDataSync().getPrimaryText();
      } catch {
        text = '';
      }
    }
    const path = this.filesDir + '/vera20k.pastecode';
    const tmp = path + '.tmp';
    try {
      const file = fileIo.openSync(tmp,
        fileIo.OpenMode.CREATE | fileIo.OpenMode.WRITE_ONLY | fileIo.OpenMode.TRUNC);
      fileIo.writeSync(file.fd, text);
      fileIo.closeSync(file);
      this.clearFile(path);
      fileIo.renameSync(tmp, path);
    } catch {
    }
    this.pasteVisible = false;
    setTimeout(() => {
      try {
        focusControl.requestFocus('vera20k_surface');
      } catch (error) {
      }
    }, 50);
  }

  // The engine has run its own quit sequence to the end; only ArkUI can
  // close the ability, which returns the player to the desktop.
  private quitToDesktop(): void {
    if (this.exiting) {
      return;
    }
    this.exiting = true;
    if (this.watchTimer >= 0) {
      clearInterval(this.watchTimer);
      this.watchTimer = -1;
    }
    const context = this.getUIContext().getHostContext();
    if (context) {
      (context as common.UIAbilityContext).terminateSelf().catch(() => {
      });
    }
  }

  build() {
    Stack() {
      if (this.engineReady) {
        XComponent({
          id: 'vera20k_surface',
          type: XComponentType.SURFACE,
          libraryname: 'vera20k_ohos'
        })
          .width('100%')
          .height('100%')
          .focusable(true)
          .focusOnTouch(true)
          .onBlur(() => {
            this.recoverSurfaceFocus();
          })
          .onMouse((event: MouseEvent) => {
            if (event.action === MouseAction.Press) {
              this.focusSurfaceNow();
            }
          })
          .onAppear(() => {
            setTimeout(() => {
              try {
                focusControl.requestFocus('vera20k_surface');
              } catch (error) {
              }
            }, 300);
          })
      } else {
        GameImport({
          onReady: () => {
            this.engineReady = true;
          }
        })
      }
      // Sits exactly over the drawn Paste row: the system checks the security
      // component's place and visibility, and the tap is the clipboard
      // authorization the engine cannot ask for itself.
      if (this.engineReady && this.pasteVisible) {
        PasteButton({
          icon: PasteIconStyle.LINES,
          text: PasteDescription.PASTE,
          buttonType: ButtonType.Normal
        })
          .fontSize(14)
          .iconSize(16)
          .width(this.pasteW)
          .height(this.pasteH)
          .position({ x: this.pasteX, y: this.pasteY })
          .onClick((event: ClickEvent, result: PasteButtonOnClickResult) => {
            this.handlePasteClick(result);
          })
      }
    }
    .width('100%')
    .height('100%')
    // The XComponent native surface is composited BELOW the ArkUI layer, so the
    // page must stay transparent where the surface should be visible.
    .backgroundColor('#00000000')
  }
}