// Language servers a project can take with one click (its environment → Language servers → Add).
// Each installs itself into `${LSP}` (Bench's folder for them on the machine) on first use:
// npm packages at fixed versions, and stub files a server reads. Rust's comes with rustup.

import type { LspSpec } from "./lsp.ts";

const STUBS = "https://raw.githubusercontent.com";

/** Intelephense's built-in stubs it is told to keep (setting `stubs` replaces its list). */
const PHP_STUBS = [
	"apache", "bcmath", "bz2", "calendar", "Core", "ctype", "curl", "date", "dom", "exif", "fileinfo", "filter", "ftp",
	"gd", "gettext", "hash", "iconv", "imagick", "intl", "json", "libxml", "mbstring", "mysqli", "mysqlnd", "openssl",
	"pcntl", "pcre", "PDO", "pdo_mysql", "Phar", "posix", "readline", "Reflection", "session", "SimpleXML", "soap",
	"sockets", "sodium", "SPL", "standard", "superglobals", "tokenizer", "xml", "xmlreader", "xmlwriter", "zip", "zlib",
	"wordpress",
];

export const LSP_PRESETS: Record<string, { title: string; spec: LspSpec }> = {
	typescript: {
		title: "TypeScript, JavaScript",
		spec: {
			command: "${LSP}/node_modules/.bin/typescript-language-server",
			args: ["--stdio"],
			extensions: ["ts", "tsx", "js", "jsx", "mjs", "cjs"],
			npm: ["typescript@5.6.3", "typescript-language-server@4.3.3"],
			initialization: { tsserver: { path: "${LSP}/node_modules/typescript/lib" } },
		},
	},
	php: {
		title: "PHP and WordPress (ACF, Polylang, WooCommerce stubs)",
		spec: {
			command: "${LSP}/node_modules/.bin/intelephense",
			args: ["--stdio"],
			extensions: ["php"],
			npm: ["intelephense@1.12.6"],
			fetch: [
				{ url: `${STUBS}/php-stubs/acf-pro-stubs/master/acf-pro-stubs.php`, to: "stubs/acf-pro-stubs.php" },
				{ url: `${STUBS}/polylang/polylang-stubs/master/polylang-stubs.php`, to: "stubs/polylang-stubs.php" },
				{ url: `${STUBS}/php-stubs/woocommerce-stubs/master/woocommerce-stubs.php`, to: "stubs/woocommerce-stubs.php" },
			],
			initialization: { globalStoragePath: "${LSP}/intelephense" },
			settings: { intelephense: { stubs: PHP_STUBS, environment: { includePaths: ["${LSP}/stubs"] }, files: { maxSize: 5000000 } } },
		},
	},
	css: {
		title: "CSS, SCSS, Less",
		spec: {
			command: "${LSP}/node_modules/.bin/vscode-css-language-server",
			args: ["--stdio"],
			extensions: ["css", "scss", "less"],
			npm: ["vscode-langservers-extracted@4.10.0"],
		},
	},
	html: {
		title: "HTML",
		spec: {
			command: "${LSP}/node_modules/.bin/vscode-html-language-server",
			args: ["--stdio"],
			extensions: ["html", "htm"],
			npm: ["vscode-langservers-extracted@4.10.0"],
		},
	},
	json: {
		title: "JSON",
		spec: {
			command: "${LSP}/node_modules/.bin/vscode-json-language-server",
			args: ["--stdio"],
			extensions: ["json", "jsonc"],
			npm: ["vscode-langservers-extracted@4.10.0"],
		},
	},
	tailwind: {
		title: "Tailwind CSS (classes in markup and styles)",
		spec: {
			command: "${LSP}/node_modules/.bin/tailwindcss-language-server",
			args: ["--stdio"],
			extensions: ["html", "css", "scss", "jsx", "tsx", "vue", "php"],
			npm: ["@tailwindcss/language-server@0.14.29"],
		},
	},
	python: {
		title: "Python",
		spec: {
			command: "${LSP}/node_modules/.bin/pyright-langserver",
			args: ["--stdio"],
			extensions: ["py"],
			npm: ["pyright@1.1.390"],
		},
	},
	rust: {
		title: "Rust (rust-analyzer from rustup)",
		spec: { command: "rust-analyzer", extensions: ["rs"] },
	},
};
