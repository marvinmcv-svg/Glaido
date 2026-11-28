import type { Preview } from "@storybook/react-vite";
import "@vocalis/design-tokens/tokens.css";
import "../src/styles.css";

const preview: Preview = {
  parameters: { layout: "padded" },
  globalTypes: {
    theme: { description: "Theme", toolbar: { items: ["dark", "light"], dynamicTitle: true } },
  },
  initialGlobals: { theme: "dark" },
  decorators: [
    (Story, ctx) => {
      document.documentElement.dataset.theme = ctx.globals.theme as string;
      document.body.style.background = "var(--bg-primary)";
      return Story();
    },
  ],
};
export default preview;
