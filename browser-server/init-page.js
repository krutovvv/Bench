// Every tab the MCP drives looks like an ordinary Chrome on Windows. Chromium's --user-agent flag
// changes only the UA string; here, over CDP, the Client Hints (Sec-CH-UA-*,
// navigator.userAgentData) and navigator.platform change too, so they don't contradict it.
exports.default = async ({ page }) => {
  const full = page.context().browser()?.version() || "155.0.0.0";
  const major = full.split(".")[0];
  const lang = process.env.BROWSER_LANG || "en-US";
  const brands = [
    { brand: "Google Chrome", version: major },
    { brand: "Chromium", version: major },
    { brand: "Not_A Brand", version: "24" },
  ];
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setUserAgentOverride", {
    userAgent: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${major}.0.0.0 Safari/537.36`,
    acceptLanguage: lang === "en-US" ? "en-US,en" : `${lang},${lang.split("-")[0]},en-US,en`,
    platform: "Win32",
    userAgentMetadata: {
      brands,
      fullVersionList: brands.map((b) => ({ ...b, version: b.brand === "Not_A Brand" ? "24.0.0.0" : full })),
      fullVersion: full,
      platform: "Windows",
      platformVersion: "19.0.0",
      architecture: "x86",
      bitness: "64",
      model: "",
      mobile: false,
      wow64: false,
    },
  });
  // The session stays attached: the override lives as long as it does.
};
