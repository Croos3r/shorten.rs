import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import axios from "axios"

import "./index.css"
import App from "./App.tsx"
import { ThemeProvider } from "@/components/theme-provider.tsx"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"

export const axiosClient = axios.create({
  baseURL: import.meta.env.VITE_API_BASE_URL,
})
const queryClient = new QueryClient()

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </ThemeProvider>
  </StrictMode>
)
