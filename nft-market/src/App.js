import React, { useState, useCallback } from 'react';

import Carousel, { Modal, ModalGateway } from 'react-images';
import Gallery from 'react-photo-gallery';
import Photo from './Photo';
import { photos } from './data.json';

/* popout the browser and maximize to see more columns! -> */
const App = () => {
  const [state, setState] = useState({ currentImage: 0, viewerIsOpen: false });
  const openLightbox = useCallback((event, { photo, index }) => {
    setState({ currentImage: index, viewerIsOpen: true });
  }, []);

  const closeLightbox = () => {
    setState({ currentImage: 0, viewerIsOpen: false });
  };

  return (
    <>
      <Gallery
        photos={photos}
        direction="column"
        onClick={openLightbox}
        renderImage={Photo}
      />
      <ModalGateway>
        {state.viewerIsOpen && (
          <Modal onClose={closeLightbox}>
            <Carousel
              currentIndex={state.currentImage}
              views={photos.map((x) => ({
                ...x,
                srcset: x.srcSet,
                caption: x.title
              }))}
            />
          </Modal>
        )}
      </ModalGateway>
    </>
  );
};

export default App;
