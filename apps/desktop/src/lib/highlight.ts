import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import go from "highlight.js/lib/languages/go";
import ini from "highlight.js/lib/languages/ini";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import kotlin from "highlight.js/lib/languages/kotlin";
import markdown from "highlight.js/lib/languages/markdown";
import php from "highlight.js/lib/languages/php";
import python from "highlight.js/lib/languages/python";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import scss from "highlight.js/lib/languages/scss";
import shell from "highlight.js/lib/languages/shell";
import sql from "highlight.js/lib/languages/sql";
import swift from "highlight.js/lib/languages/swift";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import githubLightCss from "highlight.js/styles/github.css?inline";
import githubDarkCss from "highlight.js/styles/github-dark.css?inline";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("csharp", csharp);
hljs.registerLanguage("css", css);
hljs.registerLanguage("dockerfile", dockerfile);
hljs.registerLanguage("go", go);
hljs.registerLanguage("ini", ini);
hljs.registerLanguage("java", java);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("kotlin", kotlin);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("php", php);
hljs.registerLanguage("python", python);
hljs.registerLanguage("ruby", ruby);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("scss", scss);
hljs.registerLanguage("shell", shell);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("swift", swift);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

installHljsThemes();

function installHljsThemes() {
  if (typeof document === "undefined") return;
  if (document.getElementById("hljs-themes")) return;
  const el = document.createElement("style");
  el.id = "hljs-themes";
  el.textContent =
    scopeCss(githubLightCss, '[data-theme="light"]') +
    "\n" +
    scopeCss(githubDarkCss, '[data-theme="dark"]');
  document.head.appendChild(el);
}

function scopeCss(css: string, prefix: string): string {
  const noComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
  return noComments.replace(/([^{}]+)\{([^{}]*)\}/g, (_, selectors, body) => {
    const scoped = selectors
      .split(",")
      .map((s: string) => `${prefix} ${s.trim()}`)
      .join(", ");
    return `${scoped} { ${body.trim()} }`;
  });
}

const EXT_TO_LANG: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  rs: "rust",
  py: "python",
  go: "go",
  java: "java",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hh: "cpp",
  cs: "csharp",
  rb: "ruby",
  php: "php",
  swift: "swift",
  kt: "kotlin",
  kts: "kotlin",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  json: "json",
  yml: "yaml",
  yaml: "yaml",
  toml: "ini",
  md: "markdown",
  markdown: "markdown",
  sql: "sql",
  html: "xml",
  htm: "xml",
  xml: "xml",
  svg: "xml",
  vue: "xml",
  css: "css",
  scss: "scss",
  sass: "scss",
};

export function languageFromPath(path: string): string | null {
  const file = path.split("/").pop()?.toLowerCase() ?? "";
  if (file === "dockerfile" || file.endsWith(".dockerfile")) return "dockerfile";
  const dot = file.lastIndexOf(".");
  if (dot < 0) return null;
  return EXT_TO_LANG[file.slice(dot + 1)] ?? null;
}

export function highlightToHtml(content: string, language: string | null): string {
  if (!content) return "";
  if (language && hljs.getLanguage(language)) {
    try {
      return hljs.highlight(content, { language, ignoreIllegals: true }).value;
    } catch {
      // fall through to escaped plaintext
    }
  }
  return escapeHtml(content);
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => {
    if (c === "&") return "&amp;";
    if (c === "<") return "&lt;";
    if (c === ">") return "&gt;";
    if (c === '"') return "&quot;";
    return "&#39;";
  });
}
