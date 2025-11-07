const path = require("path");
const CopyWebpackPlugin = require("copy-webpack-plugin");

module.exports = {
  entry: {
    "background/service-worker": "./src/background/service-worker.js",
    "content/claude": "./src/content/claude.js",
    "content/chatgpt": "./src/content/chatgpt.js",
    "popup/popup": "./src/popup/popup.js",
  },
  output: {
    filename: "[name].js",
    path: path.resolve(__dirname, "dist"),
    clean: true,
  },
  resolve: {
    extensions: [".js"],
  },
  module: {
    rules: [
      {
        test: /\.css$/i,
        include: path.resolve(__dirname, "src"),
        use: ["style-loader", "css-loader"],
      },
    ],
  },
  plugins: [
    new CopyWebpackPlugin({
      patterns: [
        { from: "manifest.json", to: "manifest.json" },
        { from: "icons", to: "icons" },
        { from: "src/popup/popup.html", to: "popup/popup.html" },
        { from: "src/content/styles.css", to: "content/styles.css" },
        { from: "src/shared", to: "shared", noErrorOnMissing: true },
      ],
    }),
  ],
  devtool: false,
};
