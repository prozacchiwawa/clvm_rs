// @ts-nocheck
const path = require('path')
const webpack = require('webpack')
const ReactRefreshWebpackPlugin = require('@pmmmwh/react-refresh-webpack-plugin')
const { VanillaExtractPlugin } = require('@vanilla-extract/webpack-plugin')
const HtmlWebpackPlugin = require('html-webpack-plugin')
const MiniCssExtractPlugin = require('mini-css-extract-plugin')
const ReactRefreshTypeScript = require('react-refresh-typescript').default
const ForkTsCheckerWebpackPlugin = require('fork-ts-checker-webpack-plugin')
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');

module.exports = (env) => ({
  mode: env.production ? 'production' : 'development',
  entry: {
    main: './src/main.tsx'
  },
  output: {
    filename: 'bundle.js', // output filename
    path: path.resolve(__dirname, 'dist'), // directory of where the bundle will be created at
  },
  devtool: 'source-map',
  experiments: {
    syncWebAssembly: true
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        include: [path.join(__dirname, 'src'), path.join(__dirname, '../../packages')],
        exclude: /node_modules/,
        use: [
          {
            loader: 'babel-loader',
            options: {
              babelrc: false,
              presets: [
                '@babel/preset-typescript',
                ['@babel/preset-react', { runtime: 'automatic' }],
                ['@babel/preset-env', { targets: { node: 14 }, modules: false }]
              ],
              plugins: [
                '@vanilla-extract/babel-plugin',
                env.development && 'react-refresh/babel'
              ].filter(Boolean)
            }
          }
        ].filter(Boolean)
      },
      {
        test: /\.css$/i,
        use: [MiniCssExtractPlugin.loader, 'css-loader']
      }
    ]
  },
  plugins: [
    env.development && new webpack.HotModuleReplacementPlugin(),
    env.development && new ReactRefreshWebpackPlugin(),
    new HtmlWebpackPlugin({
      filename: './index.html',
      template: './public/index.html'
    }),
    new WasmPackPlugin({
      crateDirectory: __dirname + "/..", // Define where the root of the rust code is located (where the cargo.toml file is located)
    }),
    new MiniCssExtractPlugin(),
    new VanillaExtractPlugin(),
    new ForkTsCheckerWebpackPlugin({
      typescript: {
        diagnosticOptions: {
          semantic: true,
          syntactic: true
        },
        mode: 'write-references',
        configFile: './tsconfig.json'
      }
    })
  ].filter(Boolean),
  resolve: {
    extensions: ['.js', '.ts', '.tsx', '.web.ts', '.web.tsx', '.json'],
    alias: {
      env: path.resolve(__dirname, 'src/env.js'),
      react: path.resolve(__dirname, 'node_modules/react')
    }
  }
});
