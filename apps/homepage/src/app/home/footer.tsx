import { Typography } from '@mui/material'
import React from 'react'

import { AlignCenteredContainer, Link } from './common'

export const Footer = () => (
	<AlignCenteredContainer
		maxWidth="md"
		sx={{
			flexDirection: 'column',
			justifyContent: 'center',
			height: '8rem',
			color: '#888',
			rowGap: '1rem',
		}}
	>
		<Typography sx={{ display: 'flex' }}>
			OctoBase is an
			<span
				style={{
					color: '#5085f6cc',
					margin: 'auto 0.25em',
				}}
			>
				#OpenSource
			</span>
			<span>software, built with&nbsp;</span>
			<Link href="https://www.rust-lang.org/" target="_blank" rel="noreferrer">
				Rust
			</Link>
		</Typography>
		<Typography>Copyright © 2022 Toeverything</Typography>
	</AlignCenteredContainer>
)
