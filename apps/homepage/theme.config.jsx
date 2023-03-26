import { useRouter } from 'next/router'

import Logo from './src/Logo'

export default {
	logo: <Logo />,
	head: (
		<>
			<meta name="viewport" content="width=device-width, initial-scale=1.0" />
			<meta property="og:title" content="OctoBase" />
			<meta property="og:description" content="Local-first, yet collaborative database" />
		</>
	),
	primaryHue: 208,
	project: {
		link: 'https://github.com/toeverything/octobase',
	},
	docsRepositoryBase: 'https://github.com/toeverything/octobase',
	useNextSeoProps() {
		const { asPath } = useRouter()
		if (asPath !== '/') {
			return {
				titleTemplate: '%s – OctoBase',
			}
		}
	},
	footer: {
		text: (
			<span>
				2021-{new Date().getFullYear()} ©{' '}
				<a href="https://affine.pro" target="_blank">
					Toeverything
				</a>
				.
			</span>
		),
	},
}
