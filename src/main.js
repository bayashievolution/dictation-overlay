(function () {
  'use strict';

  const caption = document.getElementById('caption');
  if (!caption) return;

  // ---- Transition (v0.3.6 〜) -------------------------------------------
  // dictation-beta のストリームモードで text が 150〜800ms 間隔で連発される。
  // テキスト全更新時に CSS アニメーションで「入ってくる感」を出す。
  // settings.transition で 5 種類の挙動を選べる。
  const TRANSITION_CLASSES = {
    'fade': 'anim-fade',
    'slide-right': 'anim-slide-right',
    'slide-left': 'anim-slide-left',
    'scroll-up': 'anim-scroll-up',
  };
  let currentTransition = 'none'; // backward compat: default OFF
  let lastText = '';
  // v0.3.24: streamMode 用、前回表示中の行配列（diff の基準）
  let lastLines = [];
  let streamMode = false;

  // Tauri 2.0 invoke API (window.__TAURI__.core.invoke)
  function getInvoke() {
    const api = window.__TAURI__;
    if (!api) return null;
    if (api.core && typeof api.core.invoke === 'function') return api.core.invoke;
    if (typeof api.invoke === 'function') return api.invoke;
    return null;
  }

  function hexToRgba(hex, alpha) {
    const m = /^#?([0-9a-fA-F]{6})$/.exec(String(hex || ''));
    if (!m) return `rgba(0,0,0,${alpha})`;
    const n = parseInt(m[1], 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
  }

  function applySettings(s) {
    if (!s || typeof s !== 'object') return;
    if (s.fontSize) caption.style.fontSize = s.fontSize + 'px';
    if (s.fontFamily) caption.style.fontFamily = s.fontFamily;
    if (s.fontWeight !== undefined) caption.style.fontWeight = s.fontWeight;
    if (s.color) caption.style.color = s.color;
    if (s.bgColor || s.bgAlpha !== undefined) {
      const alpha = (s.bgAlpha !== undefined ? Number(s.bgAlpha) : 70) / 100;
      caption.style.background = hexToRgba(s.bgColor || '#000000', alpha);
    }
    if (s.shadowOn) {
      const blur = s.shadowBlur !== undefined ? Number(s.shadowBlur) : 6;
      caption.style.textShadow = `0 0 ${blur}px ${s.shadowColor || '#000000'}`;
    } else if (s.shadowOn === false) {
      caption.style.textShadow = 'none';
    }
    if (s.strokeOn && s.strokeColor && s.strokeWidth) {
      caption.style.webkitTextStroke = `${s.strokeWidth}px ${s.strokeColor}`;
    } else if (s.strokeOn === false) {
      caption.style.webkitTextStroke = '0';
    }
    if (s.lineHeightTenth) {
      caption.style.lineHeight = (Number(s.lineHeightTenth) / 10).toString();
    }
    // v0.3.24: streamMode（YouTube 風行スクロール）。
    // true で setText が「行 diff → 新規行は下からスライドイン、消えた行は上にスライドアウト」モードに。
    if (typeof s.streamMode === 'boolean') {
      streamMode = s.streamMode;
    }
    // v0.3.16: 縦書きモード。やっさんアイデア「英語映画に日本語縦書き字幕」を機能化。
    // 値: "horizontal" (デフォルト、横書き) / "vertical-rl" (右→左、伝統的縦書き) /
    //     "vertical-lr" (左→右)。CSS の writing-mode 値そのまま受け付け。
    if (typeof s.writingMode === 'string') {
      const wm = s.writingMode;
      if (wm === 'horizontal' || wm === 'horizontal-tb') {
        caption.style.writingMode = 'horizontal-tb';
      } else if (wm === 'vertical-rl' || wm === 'vertical-lr') {
        caption.style.writingMode = wm;
      }
      // 未知値は無視（現在値維持）
    }
    // v0.3.6: transition kind
    if (typeof s.transition === 'string') {
      if (s.transition === 'none' || TRANSITION_CLASSES[s.transition]) {
        currentTransition = s.transition;
      }
    }
    // v0.3.7: borderRadius / paddingX / paddingY / blockGapTenth
    // v0.3.14: 0 ベタ送信を removeProperty で fallback 動作に
    // v0.3.15: CSS 変数経由（ var(--cap-*, fallback) ）が WebView2 で何らかの
    // 理由で効かない事故が出たため、**JS で直接 inline style に書く**方式に変更。
    // 確実性重視。CSS 変数の経路は撤去（styles.css 側もハードコードに戻す）。
    // 0 / undefined のときはデフォルト値（8/24/10/0）を inline 設定。
    if (s.borderRadius !== undefined) {
      const v = Number(s.borderRadius);
      caption.style.borderRadius = (v > 0 ? v : 8) + 'px';
    }
    if (s.paddingX !== undefined) {
      const v = Number(s.paddingX);
      const px = (v > 0 ? v : 24) + 'px';
      caption.style.paddingLeft = px;
      caption.style.paddingRight = px;
    }
    if (s.paddingY !== undefined) {
      const v = Number(s.paddingY);
      const py = (v > 0 ? v : 10) + 'px';
      caption.style.paddingTop = py;
      caption.style.paddingBottom = py;
    }
    if (s.blockGapTenth !== undefined) {
      const v = Number(s.blockGapTenth);
      // p 子要素の margin-bottom を直接書き換える（一括）
      const gap = (v > 0 ? v / 10 : 0) + 'em';
      const ps = caption.querySelectorAll('p');
      ps.forEach((p, i) => {
        if (i < ps.length - 1) p.style.marginBottom = gap;
        else p.style.marginBottom = '0';
      });
    }
  }

  // v0.3.7: text を <p> 段落に分割して挿入する。
  // dictation-beta は \n{2,} を段落区切り、\n は段落内の改行（line break）として送ってくる
  // （captions.js の renderTextIntoBox 同等のルール）。
  function applyParagraphs(text) {
    while (caption.firstChild) caption.removeChild(caption.firstChild);
    const blocks = String(text).split(/\n{2,}/);
    for (const block of blocks) {
      const p = document.createElement('p');
      const lines = block.split('\n');
      lines.forEach((line, i) => {
        if (i > 0) p.appendChild(document.createElement('br'));
        if (line.length > 0) p.appendChild(document.createTextNode(line));
      });
      caption.appendChild(p);
    }
  }

  function setText(text) {
    if (typeof text !== 'string') return;
    const changed = text !== lastText;
    lastText = text;
    if (!changed && !streamMode) return;
    if (streamMode) {
      setTextStream(text);
      return;
    }
    applyParagraphs(text);
    if (!changed) return;
    if (currentTransition === 'none') return;

    const cls = TRANSITION_CLASSES[currentTransition];
    if (!cls) return;
    Object.values(TRANSITION_CLASSES).forEach((c) => caption.classList.remove(c));
    void caption.offsetWidth;
    caption.classList.add(cls);
  }

  // v0.3.24: streamMode 用 setText。
  // text を行配列に分割し、前回 (lastLines) と末尾共通部分を比較して
  // 「上から removedCount 行が消えた、末尾に addedLines が追加された」と判定。
  // 消えた行は exit アニメ、新規行は enter アニメ。共通部分は触らない。
  // beta v0.13.31 の stream モードは「古い行が上に押し出され、新規行が下に追加」
  // パターンなので、このシンプルな diff で意図通り動く。
  const STREAM_ANIM_DURATION = 220; // CSS と合わせる
  function setTextStream(newText) {
    const newLines = String(newText).split('\n').filter((l) => l.length > 0);
    const oldLines = lastLines;

    // 末尾共通部分の長さ
    let commonSuffix = 0;
    const maxN = Math.min(newLines.length, oldLines.length);
    while (
      commonSuffix < maxN &&
      newLines[newLines.length - 1 - commonSuffix] ===
        oldLines[oldLines.length - 1 - commonSuffix]
    ) {
      commonSuffix++;
    }

    const removedCount = oldLines.length - commonSuffix;
    const addedLines = newLines.slice(0, newLines.length - commonSuffix);

    // 既存 <p> 群（リアル DOM）
    const existingPs = Array.from(caption.querySelectorAll('p'));

    // 上から removedCount 個に exit クラス、duration 後に DOM から削除
    for (let i = 0; i < removedCount && i < existingPs.length; i++) {
      const p = existingPs[i];
      p.classList.add('stream-exit');
      setTimeout(() => {
        if (p.parentNode === caption) p.parentNode.removeChild(p);
      }, STREAM_ANIM_DURATION + 40);
    }

    // 末尾に addedLines を追加（enter クラス）
    for (const line of addedLines) {
      const p = document.createElement('p');
      p.appendChild(document.createTextNode(line));
      p.classList.add('stream-enter');
      caption.appendChild(p);
      // クラスを少し遅らせて剥がす（アニメーションが完了してから）
      setTimeout(() => p.classList.remove('stream-enter'), STREAM_ANIM_DURATION + 40);
    }

    lastLines = newLines;
  }

  // v0.3.23: ResizeObserver + caption_resized 動的ウィンドウサイズ追従は撤去。
  // ウィンドウサイズは Rust 側で起動時固定。`.caption` は CSS で自由に伸び縮み。
  // やっさんの「div の入れ子で簡単と思ってた」が正解。

  // ---- v0.3.9: Fade-out on hide_caption ---------------------------------
  // hide_caption 受信で window.hide() する前に CSS フェードアウトを挟む。
  // 「字幕表示 ON」が 5 秒で beta から hide_caption が来るのにも合う。
  function fadeOutAndHide() {
    const invoke = getInvoke();
    if (!invoke) return;
    caption.classList.add('fading-out');
    setTimeout(() => {
      invoke('window_hide').catch(() => {});
      // hide した後、次に show されるときに opacity 0 のままにならないよう
      // クラスを剥がす
      caption.classList.remove('fading-out');
    }, 220);
  }

  // v0.3.12: max-width を CSS から撤去したため pinMaxWidthToScreen() は不要に
  // なった（旧 v0.3.10 で導入したが今は無害）。`.caption` の幅はテキスト + padding
  // に自然に追従し、ウィンドウは ResizeObserver で `.caption` のサイズに合わせる。

  // v0.3.15: 起動時にデフォルト値を inline style にベタ書き。
  // beta から settings が来る前から padding/角丸が見えるようにする。
  function initInlineDefaults() {
    caption.style.borderRadius = '8px';
    caption.style.paddingLeft = '24px';
    caption.style.paddingRight = '24px';
    caption.style.paddingTop = '10px';
    caption.style.paddingBottom = '10px';
  }

  function bind() {
    const api = window.__TAURI__;
    if (!api || !api.event || typeof api.event.listen !== 'function') {
      setTimeout(bind, 50);
      return;
    }
    initInlineDefaults();
    api.event.listen('show-caption', (evt) => {
      const p = evt && evt.payload;
      if (!p) return;
      // show 直後に fade-out 残留があったら即剥がす（show が来たら即不透明に戻す）
      caption.classList.remove('fading-out');
      applySettings(p.settings);
      setText(p.text);
    });
    api.event.listen('update-style', (evt) => {
      applySettings(evt && evt.payload);
    });
    // v0.3.9: hide-caption は Rust から「フェードアウトして消す」イベントとして受信
    api.event.listen('fade-out-and-hide', () => {
      fadeOutAndHide();
    });
  }

  bind();
})();
