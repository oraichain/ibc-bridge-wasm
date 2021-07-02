import React, { useState, useCallback, useEffect, useRef } from 'react';

import Carousel, { Modal, ModalGateway } from 'react-images';
import Gallery from 'react-photo-gallery';
import Photo from './Photo';

/* popout the browser and maximize to see more columns! -> */
const App = () => {
  const [state, setState] = useState({
    currentImage: 0,
    viewerIsOpen: false,
    photos: []
  });
  const openLightbox = useCallback((event, { index }) => {
    setState((props) => ({
      ...props,
      currentImage: index,
      viewerIsOpen: true
    }));
  }, []);

  const closeLightbox = () => {
    setState((props) => ({ ...props, currentImage: 0, viewerIsOpen: false }));
  };

  useEffect(() => {
    const { marketplaceContract, nftContract } = window.wasm.contracts;
    const getNFT = async () => {
      const { data } = await window.wasm.query(
        marketplaceContract,
        JSON.stringify({
          get_offerings: { limit: 100 }
        })
      );

      const {
        data: { ratio }
      } = await window.wasm.query(
        marketplaceContract,
        JSON.stringify({
          get_payment: {
            denom:
              'ibc/1D87F7F49C0E994F34935219BEB178D8D1E11DB9B94208DD0004ACA7C4E1D767'
          }
        })
      );

      const photos = await Promise.all(
        data.offerings.map(async ({ token_id, price, seller }) => {
          const { data } = await window.wasm.query(
            nftContract,
            JSON.stringify({ nft_info: { token_id } })
          );
          const { image, name, description } = data;
          const matched = description.match(/(\d+)x(\d+)/);
          let width = 1;
          let height = 1;
          if (matched) {
            width = parseInt(matched[1]);
            height = parseInt(matched[2]);
          }
          return {
            width,
            height,
            src: image,
            title: name,
            tokenId: token_id,
            price,
            // decimal is 10^6
            earthPrice: (
              window.BigInt(parseFloat(price) * 1000000) /
              window.BigInt(parseFloat(ratio) * 1000000)
            ).toString(10),
            seller
          };
        })
      );

      setState((props) => ({ ...props, photos }));
    };

    getNFT();
  }, []);

  const currentPhoto = state.photos[state.currentImage];
  const currencyRef = useRef();

  const buyNft = () => {
    alert(
      'buy this nft: ' +
        JSON.stringify(currentPhoto) +
        ' with denom ' +
        currencyRef.current.value
    );
  };

  return (
    <>
      {state.photos.length && (
        <Gallery
          photos={state.photos}
          direction="column"
          onClick={openLightbox}
          renderImage={Photo}
        />
      )}
      <ModalGateway>
        {state.viewerIsOpen && (
          <Modal onClose={closeLightbox}>
            <>
              <Photo photo={currentPhoto} buy={buyNft} />
              <form>
                <label htmlFor="currency" style={{ marginLeft: 10 }}>
                  Choose currency ({window.earthAccount.marsBalance})
                </label>

                <select name="currency" ref={currencyRef}>
                  <option value="earth">earth</option>
                  <option value="mars">mars</option>
                </select>
              </form>
            </>
          </Modal>
        )}
      </ModalGateway>
    </>
  );
};

export default App;
