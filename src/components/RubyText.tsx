import { useState, useEffect } from "react";
import { invokeOrThrow } from "../services/invoke";

interface RubyTextProps {
  text: string;
  enabled?: boolean;
  className?: string;
}

/**
 * Display Japanese text with furigana (ruby) annotations.
 * Uses lindera morphological analysis on the backend to identify kanji readings.
 */
export function RubyText({ text, enabled = true, className }: RubyTextProps) {
  const [html, setHtml] = useState<string>("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!enabled || !text.trim()) {
      setHtml(escapeHtml(text));
      return;
    }

    let cancelled = false;

    const fetchFurigana = async () => {
      setLoading(true);
      try {
        const result = await invokeOrThrow<string>("add_furigana_html", { text });
        if (!cancelled) {
          setHtml(result);
        }
      } catch {
        // Fallback: show plain text on error
        if (!cancelled) {
          setHtml(escapeHtml(text));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    fetchFurigana();

    return () => {
      cancelled = true;
    };
  }, [text, enabled]);

  if (!enabled) {
    return <span className={className}>{text}</span>;
  }

  return (
    <span
      className={className}
      dangerouslySetInnerHTML={{ __html: html }}
      style={{
        opacity: loading ? 0.7 : 1,
        transition: "opacity 0.15s",
      }}
    />
  );
}

/**
 * Inline ruby text for displaying a single word with reading.
 */
export function InlineRuby({ base, reading }: { base: string; reading: string }) {
  return (
    <ruby>
      {base}
      <rp>(</rp>
      <rt>{reading}</rt>
      <rp>)</rp>
    </ruby>
  );
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
