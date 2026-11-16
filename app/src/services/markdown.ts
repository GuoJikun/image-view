/**
 * 极简 Markdown → HTML，只服务于应用内更新说明。
 * 支持的语法刻意限定在 CHANGELOG 会用到的那几种：# 标题、- 列表、`行内代码`、**粗体**。
 * 先整行转义再拼接标签，内容来自自家 Release，仍然不允许原始 HTML 透传。
 */

const ESCAPES: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (char) => ESCAPES[char] ?? char);
}

/** 行内语法在转义之后处理，因此不会产生可执行标签。 */
function inline(text: string): string {
  return text
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
}

export function renderMarkdown(source: string): string {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const html: string[] = [];
  let paragraph: string[] = [];
  let listOpen = false;

  function flushParagraph(): void {
    if (paragraph.length === 0) return;
    html.push(`<p>${inline(paragraph.join(" "))}</p>`);
    paragraph = [];
  }

  function closeList(): void {
    if (!listOpen) return;
    html.push("</ul>");
    listOpen = false;
  }

  for (const raw of lines) {
    const line = escapeHtml(raw.trimEnd());

    if (line.trim() === "") {
      flushParagraph();
      closeList();
      continue;
    }

    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      closeList();
      /* 弹窗已有「更新内容」h3，内容里的标题整体降一级。 */
      const level = Math.min(6, heading[1].length + 1);
      html.push(`<h${level}>${inline(heading[2])}</h${level}>`);
      continue;
    }

    const item = /^[-*]\s+(.*)$/.exec(line);
    if (item) {
      flushParagraph();
      if (!listOpen) {
        html.push("<ul>");
        listOpen = true;
      }
      html.push(`<li>${inline(item[1])}</li>`);
      continue;
    }

    closeList();
    paragraph.push(line.trim());
  }

  flushParagraph();
  closeList();
  return html.join("");
}
