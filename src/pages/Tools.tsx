import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Copy,
  Check,
  RefreshCw,
  ArrowRight,
  Code,
} from "lucide-react";
import { VARIABLE_FORMATS, type VariableFormat } from "../types";

function Tools() {
  const [inputText, setInputText] = useState("");
  const [outputText, setOutputText] = useState("");
  const [targetFormat, setTargetFormat] = useState<VariableFormat>("camelCase");
  const [detectedFormat, setDetectedFormat] = useState("");
  const [copied, setCopied] = useState(false);

  const handleTransform = async () => {
    if (!inputText.trim()) return;
    try {
      const result = await invoke<string>("transform_variable_name", {
        text: inputText,
        targetFormat: targetFormat,
      });
      setOutputText(result);
    } catch (err) {
      console.error("Transform failed:", err);
    }
  };

  const handleCycle = async () => {
    if (!inputText.trim()) return;
    try {
      const [result, format] = await invoke<[string, string]>("cycle_variable_name", {
        text: inputText,
      });
      setOutputText(result);
      setDetectedFormat(format);
      setInputText(result);
    } catch (err) {
      console.error("Cycle failed:", err);
    }
  };

  const handleCopy = async () => {
    if (!outputText) return;
    try {
      await navigator.clipboard.writeText(outputText);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("Copy failed:", err);
    }
  };

  const handleUseAsInput = () => {
    if (outputText) {
      setInputText(outputText);
      setOutputText("");
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-2xl mx-auto">
        <h1 className="text-2xl font-bold mb-6">工具箱</h1>

        {/* Variable Name Transformer */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Code size={18} />
            变量名转换
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            在不同命名规范之间转换变量名：snake_case, camelCase, PascalCase, kebab-case 等
          </p>

          <div className="space-y-4">
            {/* Input */}
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                输入变量名
              </label>
              <input
                value={inputText}
                onChange={(e) => setInputText(e.target.value)}
                placeholder="my_variable_name"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>

            {/* Target Format */}
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                目标格式
              </label>
              <select
                value={targetFormat}
                onChange={(e) => setTargetFormat(e.target.value as VariableFormat)}
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary"
              >
                {VARIABLE_FORMATS.map((format) => (
                  <option key={format} value={format}>
                    {format}
                  </option>
                ))}
              </select>
            </div>

            {/* Action Buttons */}
            <div className="flex gap-2">
              <button
                onClick={handleTransform}
                disabled={!inputText.trim()}
                className="bg-primary text-bg-primary font-semibold rounded-lg px-4 py-2 text-sm hover:bg-primary-hover transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <ArrowRight size={14} />
                转换
              </button>
              <button
                onClick={handleCycle}
                disabled={!inputText.trim()}
                className="bg-bg-tertiary text-text-primary border border-border rounded-lg px-4 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <RefreshCw size={14} />
                循环切换
              </button>
            </div>

            {/* Output */}
            {outputText && (
              <div>
                <label className="block text-xs text-text-secondary mb-1.5">
                  转换结果
                  {detectedFormat && (
                    <span className="ml-2 text-accent">({detectedFormat})</span>
                  )}
                </label>
                <div className="relative">
                  <div className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm font-mono">
                    {outputText}
                  </div>
                  <div className="absolute right-2 top-1/2 -translate-y-1/2 flex gap-1">
                    <button
                      onClick={handleCopy}
                      className="p-1.5 rounded-md hover:bg-bg-primary/50 text-text-secondary hover:text-text-primary transition-colors"
                      title="复制"
                    >
                      {copied ? (
                        <Check size={14} className="text-success" />
                      ) : (
                        <Copy size={14} />
                      )}
                    </button>
                    <button
                      onClick={handleUseAsInput}
                      className="p-1.5 rounded-md hover:bg-bg-primary/50 text-text-secondary hover:text-text-primary transition-colors"
                      title="用作输入"
                    >
                      <RefreshCw size={14} />
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </section>

        {/* Quick Reference */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5">
          <h2 className="text-base font-semibold text-primary mb-4">
            命名规范参考
          </h2>
          <div className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-text-secondary">snake_case</span>
              <span className="text-text-primary font-mono">my_variable_name</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">SNAKE_CASE</span>
              <span className="text-text-primary font-mono">MY_VARIABLE_NAME</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">kebab-case</span>
              <span className="text-text-primary font-mono">my-variable-name</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">camelCase</span>
              <span className="text-text-primary font-mono">myVariableName</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">PascalCase</span>
              <span className="text-text-primary font-mono">MyVariableName</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">dot.notation</span>
              <span className="text-text-primary font-mono">my.variable.name</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">Title Case</span>
              <span className="text-text-primary font-mono">My Variable Name</span>
            </div>
          </div>
          <p className="text-xs text-text-secondary mt-4">
            快捷键：Alt+Shift+U 可快速循环切换格式
          </p>
        </section>
      </div>
    </div>
  );
}

export default Tools;
