import { styled } from '@mui/material';
import { useEffect, useMemo, useState } from 'react';

const featuresText = ['Offline available', 'Full-featured', 'Self-contained'];

export const Features = () => {
    const [idx, setIdx] = useState(0);
    const [last, current] = useMemo(
        () => [
            featuresText[idx],
            featuresText[idx + 1] ? featuresText[idx + 1] : featuresText[0],
        ],
        [idx]
    );
    const [active, setActive] = useState(false);
    useEffect(() => {
        const handle = setInterval(() => {
            setActive(true);
            setTimeout(() => {
                setIdx(idx => (featuresText[idx + 1] ? idx + 1 : 0));
                setActive(false);
            }, 380);
        }, 5000);
        return () => clearInterval(handle);
    }, []);

    return (
        <StyledContainer>
            <StyledTitle>
                <StyledLastScroll isActive={active}>
                    <div>{last}</div>
                </StyledLastScroll>
                <StyledCurrentScroll isActive={active}>
                    <div>{current}</div>
                </StyledCurrentScroll>
            </StyledTitle>
            <StyledText>collaborative database</StyledText>
        </StyledContainer>
    );
};

const StyledContainer = styled('div')({
    maxWidth: '1440px',
    width: '100%',
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    color: '#06449d',
    fontSize: '48px',
    fontWeight: '900',
    height: '100px',
    '@media (max-width: 1300px)': {
        fontSize: '40px',
    },
    '@media (max-width: 1100px)': {
        fontSize: '32px',
        height: '90px',
    },
    '@media (max-width: 900px)': {
        fontSize: '24px',
        height: '65px',
    },
    '@media (max-width: 600px)': {
        fontSize: '18px',
        height: '96px',
        flexDirection: 'column',
    },
});

const StyledTitle = styled('div')({
    position: 'relative',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'end',
    flex: 0.75,
    overflowY: 'hidden',
    height: '100%',
    paddingRight: '16px',
    '@media (max-width: 600px)': {
        flex: 1,
        width: '100%',
        paddingRight: '0px',
        justifyContent: 'center',
    },
});

interface TitleProps {
    isActive: boolean;
}

const StyledLastScroll = styled('div')<TitleProps>(({ isActive }) => ({
    display: 'flex',
    position: 'absolute',
    lineHeight: '48px',
    transition: '0.5s ease-in-out',
    opacity: '1',
    animation: isActive ? 'primaryCurrent 500ms linear infinite' : 'none',
    '@keyframes primaryLast': {
        '0%': {
            top: '25%',
            opacity: '1',
        },
        '20%': {
            top: '0%',
            opacity: '0.8',
        },
        '40%': {
            top: '-20%',
            opacity: '0.6',
        },
        '60%': {
            top: '-40%',
            opacity: '0.4',
        },
        '80%': {
            top: '-60%',
            opacity: '0.2',
        },
        '100%': {
            top: '-100%',
            opacity: '0',
        },
    },
}));

const StyledCurrentScroll = styled('div')<TitleProps>(({ isActive }) => ({
    display: 'flex',
    position: 'absolute',
    lineHeight: '48px',
    marginTop: '105px',
    transition: '0.5s ease-in-out',
    opacity: '0',
    animation: isActive ? 'primaryCurrent 500ms linear infinite' : 'none',
    '@media (max-width: 1100px)': {
        marginTop: '96px',
    },
    '@media (max-width: 900px)': {
        marginTop: '60px',
    },
    '@media (max-width: 600px)': {
        marginTop: '36px',
    },
    '@keyframes primaryCurrent': {
        from: {
            top: '0%',
            opacity: '0',
        },
        to: {
            top: '-100%',
            opacity: '1',
        },
    },
}));

const StyledText = styled('div')({
    display: 'flex',
    flex: 1,
    textAlign: 'center',
    alignItems: 'center',
});
