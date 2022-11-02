// ggl --- global git log
// Copyright (C) 2022  Honza Pokorny <honza@pokorny.ca>

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package main

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"
)

const PAGE_SIZE = 1000

// CLI flags
var Fetch bool
var Until string
var ConfigPath string

var rootCmd = &cobra.Command{
	Use:   "ggl",
	Short: "global git log",
	RunE: func(cmd *cobra.Command, args []string) error {
		return Run(args)
	},
}

type Repository struct {
	Name   string
	Path   string
	Remote string
	Branch string
	Fetch  bool
}

type Config struct {
	Repositories []Repository
	Root         string
}

type Commit struct {
	Repository
	Commit    *object.Commit
	Subject   string
	ShortHash string
}

type Commits []Commit

func (c Commits) Len() int      { return len(c) }
func (c Commits) Swap(i, j int) { c[i], c[j] = c[j], c[i] }
func (c Commits) Less(i, j int) bool {
	return c[i].Commit.Author.When.After(c[j].Commit.Author.When)
}

func loadConfig() (Config, error) {
	var config Config
	contents, err := ioutil.ReadFile(ConfigPath)
	if err != nil {
		return config, err
	}

	err = yaml.Unmarshal(contents, &config)
	if err != nil {
		return config, err
	}

	return config, nil
}

func GetGitLog(config Config, repo Repository, until time.Time) ([]Commit, error) {
	commits := []Commit{}

	repoPath := filepath.Join(config.Root, repo.Path)
	r, err := git.PlainOpen(repoPath)

	if err != nil {
		return commits, err
	}

	if Fetch && repo.Fetch {
		fmt.Printf("Fetching %s: %s/%s\n", repo.Name, repo.Remote, repo.Branch)
		err = r.Fetch(&git.FetchOptions{RemoteName: repo.Remote})
		if err != nil {
			if !errors.Is(err, git.NoErrAlreadyUpToDate) {
				return commits, err
			}
			fmt.Println("  already up-to-date")
		}
	}

	ref := fmt.Sprintf("%s/%s", repo.Remote, repo.Branch)
	revision, err := r.ResolveRevision(plumbing.Revision(ref))
	if err != nil {
		return commits, err
	}
	cIter, err := r.Log(&git.LogOptions{From: *revision, Order: git.LogOrderCommitterTime})
	if err != nil {
		return commits, err
	}
	for i := 1; i <= PAGE_SIZE; i++ {
		c, err := cIter.Next()

		if err == io.EOF {
			break
		}

		if c.Author.When.Before(until) {
			break
		}

		lines := strings.Split(c.Message, "\n")
		subject := lines[0]

		commit := Commit{
			Repository: repo,
			Commit:     c,
			Subject:    subject,
		}
		commits = append(commits, commit)
	}
	return commits, nil
}

func FormatCommit(c Commit) string {
	w := bytes.NewBufferString("")

	fmt.Fprintf(w, "commit %s\n", c.Commit.Hash)
	fmt.Fprintf(w, "Repository: %s\n", c.Repository.Name)

	parents := c.Commit.ParentHashes

	if len(parents) > 1 {
		fmt.Fprintf(w, "Merge:")
		for _, parent := range parents {
			fmt.Fprintf(w, " %s", parent.String()[:9])
		}
		fmt.Fprintf(w, "\n")
	}

	fmt.Fprintf(w, "Author: %s <%s>\n", c.Commit.Author.Name, c.Commit.Author.Email)

	commitDateFormat := "Mon Jan 2 15:04:05 2006 -0700"
	fmt.Fprintf(w, "Date:   %s\n", c.Commit.Author.When.Format(commitDateFormat))
	fmt.Fprintf(w, "\n")

	lines := strings.Split(c.Commit.Message, "\n")

	for _, line := range lines {
		fmt.Fprintln(w, "   ", line)
	}

	out := strings.TrimSpace(w.String())
	return fmt.Sprintf("%s\n", out)
}

func Run(args []string) error {
	config, err := loadConfig()
	if err != nil {
		return err
	}

	allCommits := []Commit{}

	var until time.Time

	if Until == "" {
		d, _ := time.ParseDuration("-168h") // 7 days
		until = time.Now().Add(d)
	} else {
		until, err = time.Parse("2006-01-02", Until)
		if err != nil {
			return fmt.Errorf("Failed to parse 'until' date format: expected: 2022-12-31")
		}
	}

	for _, repo := range config.Repositories {
		commits, err := GetGitLog(config, repo, until)
		if err != nil {
			return err
		}

		allCommits = append(allCommits, commits...)

	}

	sort.Sort(Commits(allCommits))

	for _, c := range allCommits {
		fmt.Println(FormatCommit(c))
	}

	return nil
}

func main() {
	rootCmd.PersistentFlags().BoolVar(&Fetch, "fetch", false, "")
	rootCmd.PersistentFlags().StringVar(&Until, "until", "", "How far back should we go?  e.g. 2022-11-01  Default: 7 days ago")
	rootCmd.PersistentFlags().StringVar(&ConfigPath, "config", "config.yaml", "")
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}
