#!/usr/bin/python

from sys import argv
from requests import get
from bs4 import BeautifulSoup


def download_impls(url):
    if url.startswith("file://"):
        with open(url[7:]) as f:
            c = f.read()
    else:
        with get(url) as v:
            c = v.content
    v = BeautifulSoup(c, "lxml").find_all("section", attrs={"class": "impl"})
    for i in v:
        k = i.find("h3").text.split("\n")[0]
        if " !" not in k:
            continue
        print(k)


if __name__ == "__main__":
    download_impls(argv[1])
