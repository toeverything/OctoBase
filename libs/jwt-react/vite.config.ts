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
		visualizer(),
	],
	build: {
		lib: {
			entry: resolve(__dirname, 'lib/index.ts'),
			name: '@toeverything/jwt-react',
			fileName: 'index',
		},
		rollupOptions: {
			external: ['jotai', 'react', 'y-protocols', 'yjs', 'flexsearch', 'sift', /^lib0/],
			output: {
				globals: {
					'@toeverything/jwt': '@toeverything/jwt',
					react: 'react',
				},
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
