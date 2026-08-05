import { useCallback, useRef, useState } from 'react';

/**
 * Pointer-based list reorder (no HTML5 DnD, which is unreliable in WebView).
 *
 * Rows must carry `data-engine-drag-id={id}`; the handle receives the props
 * returned by `dragHandleProps(id)`. While dragging, the list is reordered
 * live (swap-on-hover); `onCommit` is called once on release with the final
 * order. `order` reflects the live order while dragging, otherwise equals `ids`.
 */
export function useReorderDrag(ids: string[], onCommit: (newIds: string[]) => void) {
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [order, setOrder] = useState<string[]>(ids);
  const draggingIdRef = useRef<string | null>(null);
  const orderRef = useRef<string[]>(ids);
  const onCommitRef = useRef(onCommit);
  const moveRef = useRef<(e: PointerEvent) => void>(() => undefined);
  const endRef = useRef<() => void>(() => undefined);
  onCommitRef.current = onCommit;

  const endDrag = useCallback(() => {
    const active = draggingIdRef.current;
    draggingIdRef.current = null;
    setDraggingId(null);
    document.body.classList.remove('select-none');
    document.body.style.userSelect = '';
    window.removeEventListener('pointermove', moveRef.current);
    window.removeEventListener('pointerup', endRef.current);
    window.removeEventListener('pointercancel', endRef.current);
    if (active) onCommitRef.current(orderRef.current);
  }, []);

  const handleMove = useCallback((e: PointerEvent) => {
    const dragging = draggingIdRef.current;
    if (!dragging) return;
    const el = document.elementFromPoint(e.clientX, e.clientY);
    const row = el instanceof Element ? el.closest<HTMLElement>('[data-engine-drag-id]') : null;
    const overId = row?.dataset.engineDragId;
    if (!overId || overId === dragging) return;
    const cur = [...orderRef.current];
    const from = cur.indexOf(dragging);
    const to = cur.indexOf(overId);
    if (from < 0 || to < 0 || from === to) return;
    cur.splice(from, 1);
    cur.splice(to, 0, dragging);
    orderRef.current = cur;
    setOrder(cur);
  }, []);

  const beginDrag = useCallback(
    (e: React.PointerEvent<HTMLElement>, id: string) => {
      if (e.button !== 0) return;
      e.preventDefault();
      draggingIdRef.current = id;
      orderRef.current = ids;
      setOrder(ids);
      setDraggingId(id);
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        // capture can throw if the element was removed; drag still works
      }
      document.body.classList.add('select-none');
      document.body.style.userSelect = 'none';
      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', endDrag);
      window.addEventListener('pointercancel', endDrag);
    },
    [ids, handleMove, endDrag],
  );

  return {
    draggingId,
    order: draggingId ? order : ids,
    isDragging: (id: string) => draggingId === id,
    dragHandleProps: (id: string) => ({
      'data-engine-drag-id': id,
      onPointerDown: (e: React.PointerEvent<HTMLElement>) => beginDrag(e, id),
    }),
  };
}
