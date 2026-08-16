import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function MarkdownContent({
  value,
  fontClass = "font-inter",
  bodyClass = "text-[13px]",
  className = "",
}: {
  value: string;
  /** Override the body font. Code blocks always stay `font-mono`. */
  fontClass?: string;
  /** Override the body text size class. Default `text-[13px]`. */
  bodyClass?: string;
  /** Extra classes on the root wrapper (e.g. `selectable` to allow copying). */
  className?: string;
}) {
  return (
    <div className={`min-w-0 max-w-full break-words ${fontClass} ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          p: ({ children }) => (
            <p className={`${bodyClass} text-fg leading-relaxed mb-2`}>{children}</p>
          ),
          ul: ({ children }) => (
            <ul className={`list-disc pl-4 space-y-0.5 ${bodyClass} text-fg mb-2`}>{children}</ul>
          ),
          ol: ({ children }) => (
            <ol className={`list-decimal pl-5 space-y-0.5 ${bodyClass} text-fg mb-2`}>
              {children}
            </ol>
          ),
          li: ({ children }) => (
            <li className={`${bodyClass} text-fg leading-relaxed`}>{children}</li>
          ),
          h1: ({ children }) => <h1 className="text-sm font-semibold text-fg mb-1">{children}</h1>,
          h2: ({ children }) => <h2 className="text-sm font-semibold text-fg mb-1">{children}</h2>,
          h3: ({ children }) => (
            <h3 className={`${bodyClass} font-semibold text-fg mb-1`}>{children}</h3>
          ),
          h4: ({ children }) => (
            <h4 className={`${bodyClass} font-semibold text-fg mb-1`}>{children}</h4>
          ),
          h5: ({ children }) => (
            <h5 className={`${bodyClass} font-semibold text-fg mb-1`}>{children}</h5>
          ),
          h6: ({ children }) => (
            <h6 className={`${bodyClass} font-semibold text-fg mb-1`}>{children}</h6>
          ),
          code: ({ className, children }) => {
            const isBlock = /\blanguage-/.test(className ?? "");
            if (isBlock) {
              return <code className={`font-mono text-[12px] ${className ?? ""}`}>{children}</code>;
            }
            return (
              <code className="bg-surface px-1 py-0.5 rounded font-mono text-[12px]">
                {children}
              </code>
            );
          },
          pre: ({ children }) => (
            <pre className="bg-surface border border-border rounded p-2 font-mono text-[12px] overflow-x-auto mb-2">
              {children}
            </pre>
          ),
          a: ({ href, children }) => (
            <a
              href={href}
              className="text-blue-400 hover:underline"
              target="_blank"
              rel="noreferrer"
            >
              {children}
            </a>
          ),
          blockquote: ({ children }) => (
            <blockquote className="border-l-2 border-border pl-2 text-fg-muted my-2">
              {children}
            </blockquote>
          ),
          hr: () => <hr className="border-border my-3" />,
          table: ({ children }) => (
            <table className="border-collapse text-[13px] text-fg mb-2">{children}</table>
          ),
          th: ({ children }) => (
            <th className="border border-border px-2 py-1 text-left font-semibold">{children}</th>
          ),
          td: ({ children }) => <td className="border border-border px-2 py-1">{children}</td>,
          img: ({ src, alt }) => <img src={src} alt={alt ?? ""} className="max-w-full rounded" />,
        }}
      >
        {value}
      </ReactMarkdown>
    </div>
  );
}
