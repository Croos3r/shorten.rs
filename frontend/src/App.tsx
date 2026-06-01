import { useMutation } from "@tanstack/react-query"
import { Button } from "./components/ui/button"
import { FieldGroup } from "./components/ui/field"
import { Input } from "./components/ui/input"
import { createShortenedUrl } from "./queries"
import { useState } from "react"
import { Spinner } from "./components/ui/spinner"
import { Link } from "lucide-react"
import { isAxiosError } from "axios"

export function App() {
  const createShortenedUrlMutation = useMutation({
    mutationFn: createShortenedUrl,
    onSuccess: ({ data: id }) =>
      setUrl(`${import.meta.env.VITE_API_BASE_URL}/${id}`),
    onError: (error) => {
      if (isAxiosError(error) && error.response) {
        setError(error.response.data)
      }
    },
  })
  const [url, setUrl] = useState("")
  const [error, setError] = useState("")

  return (
    <div className="flex h-screen flex-col items-center justify-center gap-5">
      <FieldGroup className="flex flex-row items-center justify-center gap-5">
        <Input
          id="url"
          className="w-[20vw]"
          placeholder="https://example.com/any/url/"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          disabled={createShortenedUrlMutation.isPending}
        />
        <Button
          className="cursor-pointer"
          variant="outline"
          type="submit"
          onClick={() => {
            createShortenedUrlMutation.mutate(url)
          }}
          size="icon"
          disabled={createShortenedUrlMutation.isPending || url === ""}
        >
          {createShortenedUrlMutation.isPending ? <Spinner /> : <Link />}
        </Button>
      </FieldGroup>
      {error !== "" && <p className="text-red-500">{error}</p>}
    </div>
  )
}

export default App
