import { useEffect, useState, useCallback, useRef } from 'preact/hooks';
import { FluentIcon } from '../../components/fluent-icon';
import { api } from '../../lib/invoke';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEntryId } from '../../hooks/use-entry-id';
import { error as logError } from '../../lib/logger';

export function ImageViewPage() {
  const entryId = useEntryId();
  const [imageSrc, setImageSrc] = useState('');
  const [error, setError] = useState('');
  const [imageIds, setImageIds] = useState<number[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);

  // Zoom + drag state via ref to avoid re-renders on every mouse move
  const zoomRef = useRef({ scale: 1, tx: 0, ty: 0, dragging: false, lastX: 0, lastY: 0 });
  const [, forceRender] = useState(0);
  const imgRef = useRef<HTMLImageElement>(null);

  // Load image list for navigation
  useEffect(() => {
    if (!entryId) return;
    api.getImageList(0, '').then((ids) => {
      setImageIds(ids);
      const idx = ids.indexOf(entryId);
      setCurrentIndex(idx >= 0 ? idx : 0);
    }).catch((e) => logError('ImageViewPage loadImage', e));
  }, [entryId]);

  const loadImage = useCallback((id: number) => {
    api.getEntryImageFull(id)
      .then((filePath) => setImageSrc(convertFileSrc(filePath)))
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (!entryId) { setError('无效的条目 ID'); return; }
    loadImage(entryId);
  }, [entryId, loadImage]);

  const resetZoom = () => {
    const z = zoomRef.current;
    z.scale = 1; z.tx = 0; z.ty = 0; z.dragging = false;
    forceRender((n) => n + 1);
  };

  const goTo = (direction: -1 | 1) => {
    if (imageIds.length <= 1) return;
    const newIndex = (currentIndex + direction + imageIds.length) % imageIds.length;
    setCurrentIndex(newIndex);
    const newId = imageIds[newIndex];
    resetZoom();
    loadImage(newId);
    window.history.replaceState(null, '', `#/viewer/image?id=${newId}`);
  };

  // Keyboard navigation
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') { e.preventDefault(); goTo(-1); }
      if (e.key === 'ArrowRight') { e.preventDefault(); goTo(1); }
      if (e.key === 'Escape') { window.history.back(); }
      if (e.key === '0' || e.key === 'Home') { e.preventDefault(); resetZoom(); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [imageIds, currentIndex]);

  if (error) return <div class="viewer-error">{error}</div>;
  if (!imageSrc) return <div class="viewer-loading">加载中...</div>;

  const z = zoomRef.current;

  return (
    <div
      class="viewer-page image-viewer"
      style={{ background: 'var(--color-bg)' }}
      onWheel={(e) => {
        e.preventDefault();
        const s = zoomRef.current;
        const delta = e.deltaY > 0 ? -0.15 : 0.15;
        s.scale = Math.max(0.3, Math.min(10, s.scale + delta));
        if (s.scale <= 1) { s.tx = 0; s.ty = 0; }
        forceRender((n) => n + 1);
      }}
      onMouseDown={(e) => {
        if (zoomRef.current.scale <= 1) return;
        const s = zoomRef.current;
        s.dragging = true; s.lastX = e.clientX; s.lastY = e.clientY;
      }}
      onMouseMove={(e) => {
        const s = zoomRef.current;
        if (!s.dragging || s.scale <= 1) return;
        s.tx += (e.clientX - s.lastX) / s.scale;
        s.ty += (e.clientY - s.lastY) / s.scale;
        s.lastX = e.clientX; s.lastY = e.clientY;
        forceRender((n) => n + 1);
      }}
      onMouseUp={() => { zoomRef.current.dragging = false; }}
      onMouseLeave={() => { zoomRef.current.dragging = false; }}
    >
      <div class="viewer-toolbar" style={{ position: 'fixed', top: 0, left: 0, right: 0, zIndex: 10, background: 'var(--color-surface)' }}>
        <div class="viewer-toolbar-icon">
          <FluentIcon name="image" size={20} />
        </div>
        <span class="viewer-title">图片查看</span>
        {imageIds.length > 1 && (
          <span style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>{currentIndex + 1} / {imageIds.length}</span>
        )}
        {z.scale !== 1 && (
          <button class="viewer-btn" onClick={resetZoom} style={{ marginLeft: 'auto' }}>
            重置缩放
          </button>
        )}
      </div>

      <div class="image-container">
        {imageIds.length > 1 && currentIndex > 0 && (
          <button class="nav-btn prev" onClick={() => goTo(-1)} title="上一张">
            <FluentIcon name="chevronLeft" size={24} />
          </button>
        )}
        <img
          ref={imgRef}
          src={imageSrc}
          alt={`Entry ${entryId}`}
          draggable={false}
          style={{
            cursor: z.dragging ? 'grabbing' : (z.scale > 1 ? 'grab' : 'default'),
            transition: 'transform 0.1s ease-out',
            transform: `scale(${z.scale}) translate(${z.tx}px, ${z.ty}px)`,
            maxWidth: '90vw',
            maxHeight: '90vh',
            objectFit: 'contain',
          }}
        />
        {imageIds.length > 1 && currentIndex < imageIds.length - 1 && (
          <button class="nav-btn next" onClick={() => goTo(1)} title="下一张">
            <FluentIcon name="chevronRight" size={24} />
          </button>
        )}
      </div>
    </div>
  );
}
