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
			preventAssignment: true,
			JWT_DEV: true,
		}),
		visualizer(),
	],
	build: {
		lib: {
			entry: {
				index: resolve(__dirname, 'lib/index.ts'),
				'rpc/keck': resolve(__dirname, 'lib/rpc/keck.ts'),
			},
			name: '@toeverything/jwt',
		},
		rollupOptions: {
			external: ['y-protocols', 'yjs', 'flexsearch', 'sift', /^lib0/],
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
