import { Box, Container, Typography } from '@mui/material';

import { Features } from './features';
import { Footer } from './footer';
import { Header } from './header';

const Body = () => {
    return (
        <>
            <Typography variant="h2">
                {/* Sync to everywhere, Embedded everywhere */}
                {/* one liner hasn't been decided yet  */}
            </Typography>

            <Features />
            <Box
                sx={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                }}
            >
                <Typography variant="h6">
                    Offline available, full-featured, self-contained
                </Typography>
                <Typography variant="h6">
                    Offline editing with CRDTs support
                </Typography>
                <Typography variant="h6">
                    Integrated fine-grained permissions management
                </Typography>
                <Typography variant="h6">
                    Built-in powerful data processing/analysis features
                </Typography>
                <Typography variant="h6">
                    Full text indexing/searching
                </Typography>
                <Typography variant="h6">Data analytic</Typography>
                <Typography variant="h6">Workflow automation</Typography>
                <Typography variant="h6">
                    Rich Text/Rich Media Editing
                </Typography>
                <Typography variant="h6">
                    Native multi-platform, multi-storage backend support
                </Typography>
                <Typography variant="h6">
                    Read and write structured and binary data with one set of
                    apis
                </Typography>
                <Typography variant="h6">
                    Cloud-Native / On-Premises / Embedded support out of the box
                </Typography>
                <Typography variant="h6">
                    Synchronize data using multiple protocols
                </Typography>
            </Box>
        </>
    );
};

export const HomePage = () => {
    return (
        <>
            <Header />
            <Container>
                <Body />
                <Footer />
            </Container>
        </>
    );
};
