# Left Hand Control — Documentation & Landing Site

Starter documentation and landing website for Left Hand Control, built with [VitePress](https://vitepress.dev/) and [`vitepress-theme-neptu-landing`](https://github.com/bozonx/vitepress-theme-neptu).

## Requirements

- Node.js >= 22
- npm >= 10

## Getting Started

1. **Install dependencies:**

   ```bash
   npm install
   ```

2. **Start development server:**

   ```bash
   npm run dev
   ```

   Open [http://localhost:5173/](http://localhost:5173/) in your browser.

3. **Build for production:**

   ```bash
   npm run build
   ```

4. **Preview production build:**

   ```bash
   npm run preview
   ```

5. **Typecheck:**

   ```bash
   npm run typecheck
   ```

## Structure

```
site/
├── package.json
├── tsconfig.json
├── .gitignore
├── README.md
└── src/
    ├── site.yaml              # Global site configuration & theme defaults
    ├── env.d.ts
    ├── index.md               # Root language selector
    ├── .vitepress/
    │   ├── config.ts          # VitePress configuration & metadata
    │   └── theme/
    │       ├── index.ts       # Theme registration
    │       ├── Layout.vue     # Theme layout with color/style pickers
    │       └── styles.css     # Semantic token overrides
    ├── public/                # Static assets (favicons, logos, manifests)
    │   ├── favicon.ico
    │   ├── favicon.svg
    │   ├── site.webmanifest
    │   ├── robots.txt
    │   ├── img/
    │   └── admin/             # Sveltia CMS configuration
    ├── en/
    │   ├── _site.yaml         # English locale metadata, nav & sidebar
    │   ├── index.md           # English landing page
    │   └── doc/               # English documentation pages
    │       └── index.md       # Docs overview
    └── ru/
        ├── _site.yaml         # Russian locale metadata, nav & sidebar
        ├── index.md           # Russian landing page
        └── doc/               # Russian documentation pages
            └── index.md       # Docs overview
```

## Adding Documentation Pages

1. Add your markdown file to `src/en/doc/<page-name>.md` and `src/ru/doc/<page-name>.md`.
2. Register the page in the sidebar in `src/en/_site.yaml` and `src/ru/_site.yaml` under `themeConfig.sidebar.doc`.
