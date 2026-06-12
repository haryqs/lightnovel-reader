// 阅读主题定义。
// 注意：epub 正文是渲染在一个 iframe 里的，外层页面的 CSS 进不去，
// 所以正文的配色必须通过 rendition.themes 单独注入。
// 这里的颜色要和 styles.css 里 body[data-theme=...] 保持一致，视觉才统一。

export type ThemeName = "light" | "sepia" | "dark";

// 每套主题注入到 epub 正文 iframe 的样式
export const readerThemes: Record<ThemeName, Record<string, Record<string, string>>> = {
  light: {
    body: {
      background: "#ffffff",
      color: "#1a1a1a",
    },
    a: { color: "#3b6ea5 !important" },
  },
  sepia: {
    body: {
      background: "#f5ecd9",
      color: "#5b4636",
    },
    a: { color: "#9c6b3f !important" },
  },
  dark: {
    body: {
      background: "#1e1e1e",
      color: "#c9c4bb",
    },
    a: { color: "#6fa8d6 !important" },
  },
};

// 正文通用排版（所有主题共用），让阅读更舒适
export const baseTypography: Record<string, Record<string, string>> = {
  body: {
    "line-height": "1.7",
    "padding": "0 8px",
    "text-align": "justify",
  },
  p: {
    "margin": "0.8em 0",
  },
};
