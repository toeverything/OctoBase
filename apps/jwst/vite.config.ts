import react from '@vitejs/plugin-react'
import { resolve } from 'node:path'
import { visualizer } from 'rollup-plugin-visualizer'
import { defineConfig } from 'vite'
import { createHtmlPlugin } from 'vite-plugin-html'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig(() => {
	return {
		resolve: {
			alias: [
				{
					find: '@',
					replacement: resolve(__dirname, ''),
				},
			],
		},
		plugins: [
			react(),
			viteSingleFile({ removeViteModuleLoader: true }),
			createHtmlPlugin({
				minify: true,
			}),
			visualizer(),
		],
		build: {
			rollupOptions: {
				output: {
					format: 'es',
				},
			},
			minify: 'terser',
			terserOptions: {
				output: {
					comments: false,
				},
			},
		},
	}
})
