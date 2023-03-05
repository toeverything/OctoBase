import replace from '@rollup/plugin-replace'
import { resolve } from 'path'
import rollupTs from 'rollup-plugin-typescript2'
import { visualizer } from 'rollup-plugin-visualizer'
import { defineConfig } from 'vite'

export default defineConfig({
	plugins: [
		{
			...rollupTs({
				check: true,
				tsconfig: './tsconfig.json',
				tsconfigOverride: {
					noEmits: true,
				},
			}),
			// run before build
			enforce: 'pre',
		},
		replace({
			JWT_DEV: true,
		}),
		visualizer(),
	],
	build: {
		lib: {
			entry: resolve(__dirname, 'lib/index.ts'),
			name: '@toeverything/jwt',
			fileName: 'index',
		},
		rollupOptions: {
			external: [],
			output: {
				globals: {},
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
})
