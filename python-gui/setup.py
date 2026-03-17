"""
Setup script for Chat Client Advanced

Install with: python setup.py install
Or develop with: python setup.py develop
"""

from setuptools import setup, find_packages

with open("README.md", "r", encoding="utf-8") as fh:
    long_description = fh.read()

setup(
    name="chat-client-advanced",
    version="2.0.0",
    author="Chat Client Contributors",
    description="Modern Python GUI client for Rust messaging server",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/chat-client",
    packages=find_packages(),
    classifiers=[
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.6",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Programming Language :: Python :: 3.13",
        "Programming Language :: Python :: 3.14",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Development Status :: 5 - Production/Stable",
        "Intended Audience :: End Users/Desktop",
        "Topic :: Communications :: Chat",
    ],
    python_requires=">=3.6",
    entry_points={
        "console_scripts": [
            "chat-client=main:main",
        ],
    },
    keywords="chat client gui tkinter messaging",
)
