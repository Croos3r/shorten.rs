import { axiosClient } from "./main"

export const createShortenedUrl = (url: string) =>
  axiosClient.post("/shorten?" + new URLSearchParams({ url }).toString())
