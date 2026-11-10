import { getContext, setContext } from "svelte";

type ScrollGetter = () => HTMLElement | undefined;
const KEY = Symbol("scroll");

/** The shell gives its scrolling content element to the screens it shows. */
export const provideScroll = (get: ScrollGetter) => setContext(KEY, get);
/** Call during component setup; the returned getter gives the element once it is mounted. */
export const useScroll = () => getContext<ScrollGetter | undefined>(KEY);
